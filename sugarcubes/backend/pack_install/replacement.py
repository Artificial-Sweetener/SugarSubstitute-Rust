#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Replace only the tracked files of a pack, journaled so it survives a power cut.

The journal moves through three phases. `backing-up`: nothing in the pack has
changed yet, so recovery only discards the backup. `replacing`: the pack may be
half replaced, so recovery restores every backed-up file. `applied`: the new
version is in place and waits for its first successful start; until that start
confirms it, recovery treats the update as failed and restores the backup.
Untracked files (user data, caches) are never touched, except that an untracked
file sitting where the new release puts a file is backed up and restored too.
"""

from __future__ import annotations

import logging
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

from .entry_paths import TRACKING_FILE_NAME, EntryName, parse_entry
from .errors import PackInstallError, PackInstallErrorCode
from .filesystem import durable_copy, remove_file, remove_tree
from .json_records import read_record, text_field, text_list_field, write_record
from .tracking import write_tracking

_logger = logging.getLogger("sugarcubes.pack_install")
_JOURNAL_NAME = "journal.json"
_BACKUP_FILES = "files"
_BACKUP_TRACKING = "tracking"


class JournalPhase(str, Enum):
    """Name how far a replacement got before the process stopped."""

    BACKING_UP = "backing-up"
    REPLACING = "replacing"
    APPLIED = "applied"


@dataclass(frozen=True, slots=True)
class FileOperations:
    """Name every mutating primitive so tests can fail or crash at each one."""

    copy: Callable[[Path, Path], None] = durable_copy
    remove: Callable[[Path], None] = remove_file
    write_tracking: Callable[[Path, tuple[EntryName, ...]], None] = write_tracking


@dataclass(frozen=True, slots=True)
class ReplacementPlan:
    """Describe one replacement of an owned pack by a staged release."""

    target: Path
    source: Path
    old_entries: tuple[EntryName, ...]
    new_entries: tuple[EntryName, ...]
    from_version: str
    to_version: str


@dataclass(frozen=True, slots=True)
class PendingReplacement:
    """Describe a journaled replacement found on disk."""

    phase: JournalPhase
    from_version: str
    to_version: str


@dataclass(slots=True)
class TrackedReplacement:
    """Own the journaled replacement transaction in one transaction directory."""

    transaction_root: Path
    operations: FileOperations = field(default_factory=FileOperations)

    def pending(self) -> PendingReplacement | None:
        """Return the journaled replacement, if one is on disk."""

        record = read_record(self.transaction_root / _JOURNAL_NAME)
        if record is None:
            return None
        return PendingReplacement(
            phase=JournalPhase(text_field(record, "phase")),
            from_version=text_field(record, "fromVersion"),
            to_version=text_field(record, "toVersion"),
        )

    def apply(self, plan: ReplacementPlan) -> None:
        """Replace the tracked files and leave the journal in the applied phase.

        Any failure restores the previous files before the error propagates.
        """

        if self.transaction_root.exists():
            raise PackInstallError(
                PackInstallErrorCode.BUSY, "An earlier update has not been settled."
            )
        self.transaction_root.mkdir(parents=True)
        try:
            self._write_journal(plan, JournalPhase.BACKING_UP, ())
            backed_up = self._back_up(plan)
        except Exception:
            # Nothing in the pack has changed yet, so dropping the backup is enough.
            _logger.warning(
                "SugarCubes update event=backup_failed from=%s to=%s",
                plan.from_version,
                plan.to_version,
            )
            self.discard_unstarted()
            raise
        self._write_journal(plan, JournalPhase.REPLACING, backed_up)
        try:
            self._replace(plan)
            self._write_journal(plan, JournalPhase.APPLIED, backed_up)
        except Exception:
            # A transaction guard: any failure, expected or not, restores the pack.
            _logger.exception(
                "SugarCubes update event=replacement_failed from=%s to=%s",
                plan.from_version,
                plan.to_version,
            )
            self.roll_back(plan.target)
            raise

    def confirm(self) -> None:
        """Settle an applied replacement after the new version has started."""

        pending = self.pending()
        if pending is None or pending.phase is not JournalPhase.APPLIED:
            return
        remove_tree(self.transaction_root)

    def discard_unstarted(self) -> None:
        """Remove a journal whose backup began but whose pack is untouched."""

        remove_tree(self.transaction_root)

    def roll_back(self, target: Path) -> None:
        """Restore the backed-up files; safe to repeat after an interruption."""

        record = read_record(self.transaction_root / _JOURNAL_NAME)
        if record is None:
            return
        old_entries = _entries(record, "oldEntries")
        new_entries = _entries(record, "newEntries")
        backup = self.transaction_root / _BACKUP_FILES
        for entry in new_entries:
            if not entry.is_directory:
                self.operations.remove(_under(target, entry.path))
        for path in text_list_field(record, "backedUp"):
            destination = _under(target, path)
            destination.parent.mkdir(parents=True, exist_ok=True)
            self.operations.copy(_under(backup, path), destination)
        tracking_backup = self.transaction_root / _BACKUP_TRACKING
        if tracking_backup.is_file():
            self.operations.copy(tracking_backup, target / TRACKING_FILE_NAME)
        else:
            self.operations.remove(target / TRACKING_FILE_NAME)
        _remove_empty_directories(target, new_entries, keep=old_entries)
        remove_tree(self.transaction_root)

    def _back_up(self, plan: ReplacementPlan) -> tuple[str, ...]:
        """Copy every file the replacement could overwrite or delete."""

        new_files = {
            entry.folded for entry in plan.new_entries if not entry.is_directory
        }
        candidates = [
            entry.path for entry in plan.old_entries if not entry.is_directory
        ]
        candidates.extend(
            entry.path for entry in plan.new_entries if not entry.is_directory
        )
        backed_up: list[str] = []
        seen: set[str] = set()
        backup = self.transaction_root / _BACKUP_FILES
        for path in candidates:
            if path.casefold() in seen:
                continue
            seen.add(path.casefold())
            installed = _under(plan.target, path)
            if installed.is_dir() and path.casefold() in new_files:
                raise PackInstallError(
                    PackInstallErrorCode.TARGET_CHANGED,
                    f"A folder sits where the update puts the file {path!r}.",
                )
            if installed.is_file():
                destination = _under(backup, path)
                destination.parent.mkdir(parents=True, exist_ok=True)
                self.operations.copy(installed, destination)
                backed_up.append(path)
        tracking = plan.target / TRACKING_FILE_NAME
        if tracking.is_file():
            self.operations.copy(tracking, self.transaction_root / _BACKUP_TRACKING)
        return tuple(backed_up)

    def _replace(self, plan: ReplacementPlan) -> None:
        """Remove files the new release drops, then install its files and record."""

        new_keys = {entry.folded for entry in plan.new_entries}
        for entry in plan.old_entries:
            if not entry.is_directory and entry.folded not in new_keys:
                self.operations.remove(_under(plan.target, entry.path))
        _remove_empty_directories(plan.target, plan.old_entries, keep=plan.new_entries)
        for entry in plan.new_entries:
            destination = _under(plan.target, entry.path)
            if entry.is_directory:
                destination.mkdir(parents=True, exist_ok=True)
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            self.operations.copy(_under(plan.source, entry.path), destination)
        self.operations.write_tracking(plan.target, plan.new_entries)

    def _write_journal(
        self,
        plan: ReplacementPlan,
        phase: JournalPhase,
        backed_up: tuple[str, ...],
    ) -> None:
        """Durably record the transaction's phase before the next step runs."""

        write_record(
            self.transaction_root / _JOURNAL_NAME,
            {
                "phase": phase.value,
                "fromVersion": plan.from_version,
                "toVersion": plan.to_version,
                "oldEntries": [entry.text for entry in plan.old_entries],
                "newEntries": [entry.text for entry in plan.new_entries],
                "backedUp": list(backed_up),
            },
        )


def _entries(record: dict[str, object], key: str) -> tuple[EntryName, ...]:
    """Return validated entries from a journal field."""

    return tuple(parse_entry(text) for text in text_list_field(record, key))


def _under(root: Path, entry_path: str) -> Path:
    """Join a validated forward-slash entry onto a root."""

    return root.joinpath(*entry_path.split("/"))


def _remove_empty_directories(
    root: Path, entries: tuple[EntryName, ...], *, keep: tuple[EntryName, ...]
) -> None:
    """Remove now-empty directories the given entries implied, deepest first."""

    kept = {entry.folded for entry in keep if entry.is_directory}
    kept.update(
        "/".join(entry.folded.split("/")[:depth])
        for entry in keep
        for depth in range(1, len(entry.folded.split("/")))
    )
    candidates: set[str] = set()
    for entry in entries:
        parts = entry.path.split("/")
        limit = len(parts) if entry.is_directory else len(parts) - 1
        candidates.update("/".join(parts[:depth]) for depth in range(1, limit + 1))
    for path in sorted(candidates, key=lambda item: item.count("/"), reverse=True):
        if path.casefold() in kept:
            continue
        directory = _under(root, path)
        if directory.is_dir() and not any(directory.iterdir()):
            directory.rmdir()


__all__ = [
    "FileOperations",
    "JournalPhase",
    "PendingReplacement",
    "ReplacementPlan",
    "TrackedReplacement",
]
