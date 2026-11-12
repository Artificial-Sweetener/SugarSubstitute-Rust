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
"""Install a chosen update while ComfyUI starts, before the pack is imported.

ComfyUI runs prestartup scripts before it imports any custom node, so nothing of
SugarCubes is loaded yet and the backend is never replaced under a running
session. This step needs no network. It first settles any earlier replacement:
an interrupted one, or one whose new version never confirmed a successful start,
is rolled back. Then it installs the release the user chose, if every check
still holds. The new version is accepted only after a complete ComfyUI start and
its first page load; a start or page that fails restores the previous version.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from .errors import PackInstallError, PackInstallErrorCode
from .file_lock import exclusive_update_lock
from .json_records import read_record, write_record
from .ownership import (
    NotOwnedPack,
    OwnedPack,
    inspect_pack_folder,
    installed_file_digests,
)
from .replacement import (
    FileOperations,
    JournalPhase,
    ReplacementPlan,
    TrackedReplacement,
)
from .staging import MISSING_FILE_DIGEST, StagedRelease, StagedStore

_logger = logging.getLogger("sugarcubes.pack_install")
_OUTCOME = "outcome.json"
_TRANSACTION = "transaction"


class StartupOutcomeKind(str, Enum):
    """Name what the last start did with a chosen update."""

    INSTALLED = "installed"
    RESTORED = "restored"
    NOT_INSTALLED = "not-installed"


@dataclass(frozen=True, slots=True)
class StartupOutcome:
    """Describe the result the Updates entry reports after a start."""

    kind: StartupOutcomeKind
    version: str
    from_version: str
    reason: str


def updates_root(pack_folder: Path) -> Path:
    """Return the pack's private, untracked updates directory."""

    return pack_folder / ".sugarcubes" / "updates"


def run_startup_update(
    pack_folder: Path, operations: FileOperations = FileOperations()
) -> StartupOutcome | None:
    """Settle earlier work, then install the chosen release if it is still valid."""

    root = updates_root(pack_folder)
    if not root.is_dir():
        return None
    with exclusive_update_lock(root):
        replacement = TrackedReplacement(root / _TRANSACTION, operations)
        outcome = _settle_earlier_replacement(pack_folder, replacement)
        if outcome is None:
            outcome = _install_chosen_release(pack_folder, root, replacement)
        if outcome is not None:
            write_outcome(root, outcome)
        return outcome


def accept_backend_start(
    pack_folder: Path, running_version: str
) -> StartupOutcome | None:
    """Record that a just-installed version completed a full ComfyUI start.

    Call only once ComfyUI is listening with every SugarCubes node registered.
    The backup is kept until the version's first page load reports in.
    """

    root = updates_root(pack_folder)
    if not root.is_dir():
        return None
    with exclusive_update_lock(root):
        replacement = TrackedReplacement(root / _TRANSACTION)
        pending = replacement.pending()
        if (
            pending is None
            or pending.phase is not JournalPhase.APPLIED
            or pending.to_version != running_version
        ):
            return None
        replacement.advance(JournalPhase.STARTED)
        outcome = StartupOutcome(
            StartupOutcomeKind.INSTALLED, pending.to_version, pending.from_version, ""
        )
        write_outcome(root, outcome)
        _logger.info(
            "SugarCubes update event=installed version=%s from=%s awaiting=page",
            pending.to_version,
            pending.from_version,
        )
        return outcome


def report_page_load(pack_folder: Path, running_version: str, *, loaded: bool) -> bool:
    """Settle a started version by its first page load; returns whether it applied.

    A page that loaded confirms the update and drops the backup. A page that
    reports a load failure marks the update failed, so the next start restores
    the previous version.
    """

    root = updates_root(pack_folder)
    if not root.is_dir():
        return False
    with exclusive_update_lock(root):
        replacement = TrackedReplacement(root / _TRANSACTION)
        pending = replacement.pending()
        if (
            pending is None
            or pending.phase is not JournalPhase.STARTED
            or pending.to_version != running_version
        ):
            return False
        if loaded:
            replacement.confirm()
            _logger.info(
                "SugarCubes update event=page_loaded version=%s", running_version
            )
        else:
            replacement.advance(JournalPhase.PAGE_FAILED)
            _logger.warning(
                "SugarCubes update event=page_failed version=%s action=restore_at_next_start",
                running_version,
            )
        return True


def read_outcome(root: Path) -> StartupOutcome | None:
    """Return the last recorded startup outcome, if any."""

    record = read_record(root / _OUTCOME)
    if record is None:
        return None
    return StartupOutcome(
        kind=StartupOutcomeKind(str(record.get("kind"))),
        version=str(record.get("version", "")),
        from_version=str(record.get("fromVersion", "")),
        reason=str(record.get("reason", "")),
    )


def write_outcome(root: Path, outcome: StartupOutcome) -> None:
    """Record one startup outcome for the Updates entry."""

    write_record(
        root / _OUTCOME,
        {
            "kind": outcome.kind.value,
            "version": outcome.version,
            "fromVersion": outcome.from_version,
            "reason": outcome.reason,
        },
    )


def clear_outcome(root: Path) -> None:
    """Forget the last outcome once the user has seen it."""

    (root / _OUTCOME).unlink(missing_ok=True)


def _settle_earlier_replacement(
    pack_folder: Path, replacement: TrackedReplacement
) -> StartupOutcome | None:
    """Roll back an interrupted or unconfirmed replacement left by an earlier start."""

    pending = replacement.pending()
    if pending is None:
        return None
    if pending.phase is JournalPhase.BACKING_UP:
        replacement.discard_unstarted()
        return None
    if pending.phase is JournalPhase.STARTED:
        # The version started; no page has loaded yet. A newer chosen update
        # supersedes the wait, otherwise the backup stays for the first page.
        if StagedStore(updates_root(pack_folder)).has_schedule():
            replacement.confirm()
        return None
    replacement.roll_back(pack_folder)
    interrupted = pending.phase is JournalPhase.REPLACING
    if interrupted:
        # The release stays ready; installing it again is the user's choice.
        StagedStore(updates_root(pack_folder)).unschedule()
    reason = {
        JournalPhase.REPLACING: "interrupted",
        JournalPhase.PAGE_FAILED: "page-did-not-load",
    }.get(pending.phase, "did-not-start")
    _logger.warning(
        "SugarCubes update event=restored version=%s from=%s reason=%s",
        pending.to_version,
        pending.from_version,
        reason,
    )
    return StartupOutcome(
        StartupOutcomeKind.RESTORED, pending.to_version, pending.from_version, reason
    )


def _install_chosen_release(
    pack_folder: Path, root: Path, replacement: TrackedReplacement
) -> StartupOutcome | None:
    """Install the staged release the user chose, refusing anything that changed."""

    store = StagedStore(root)
    store.clear_partials()
    if not store.has_schedule():
        return None
    try:
        release = store.load()
    except PackInstallError as exc:
        store.discard()
        return _not_installed("", "", exc.code.value)
    if release is None or not store.scheduled_for(release):
        store.unschedule()
        return None
    ownership = inspect_pack_folder(pack_folder)
    if isinstance(ownership, NotOwnedPack):
        store.unschedule()
        return _not_installed(
            release.version, release.from_version, ownership.reason.value
        )
    refusal = _refusal(store, release, ownership)
    if refusal is not None:
        store.discard()
        return _not_installed(release.version, release.from_version, refusal)
    try:
        replacement.apply(
            ReplacementPlan(
                target=pack_folder,
                source=release.files_root,
                old_entries=ownership.entries,
                new_entries=release.entries,
                from_version=release.from_version,
                to_version=release.version,
            )
        )
    except (OSError, PackInstallError) as exc:
        store.unschedule()
        _logger.warning(
            "SugarCubes update event=install_failed version=%s reason=%s",
            release.version,
            type(exc).__name__,
        )
        return _not_installed(release.version, release.from_version, "install-failed")
    store.discard()
    _logger.info(
        "SugarCubes update event=applied version=%s from=%s awaiting=start",
        release.version,
        release.from_version,
    )
    return StartupOutcome(
        StartupOutcomeKind.INSTALLED,
        release.version,
        release.from_version,
        "awaiting-start",
    )


def _refusal(store: StagedStore, release: StagedRelease, pack: OwnedPack) -> str | None:
    """Return why the chosen release can no longer be installed safely, if it can't."""

    if str(pack.identity.version) != release.from_version:
        return PackInstallErrorCode.TARGET_CHANGED.value
    installed = {
        path: digest or MISSING_FILE_DIGEST
        for path, digest in installed_file_digests(pack).items()
    }
    if installed != release.installed_digests:
        return "changed-files"
    try:
        store.verify(release)
    except PackInstallError as exc:
        return exc.code.value
    return None


def _not_installed(version: str, from_version: str, reason: str) -> StartupOutcome:
    """Build the outcome for a chosen update that was left uninstalled."""

    _logger.warning(
        "SugarCubes update event=not_installed version=%s reason=%s", version, reason
    )
    return StartupOutcome(
        StartupOutcomeKind.NOT_INSTALLED, version, from_version, reason
    )


__all__ = [
    "StartupOutcome",
    "StartupOutcomeKind",
    "clear_outcome",
    "accept_backend_start",
    "read_outcome",
    "report_page_load",
    "run_startup_update",
    "updates_root",
    "write_outcome",
]
