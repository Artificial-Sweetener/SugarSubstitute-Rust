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
"""Keep one verified, extracted release ready to install at the next start.

A release is extracted into a private directory, checked, described by a
manifest, and only then renamed into place, so a half-staged release is never
mistaken for a ready one. The manifest also records the digests of the installed
tracked files at staging time: installing later proves nothing changed since.
The user's choice to install is a separate record, so staging never implies it.
"""

from __future__ import annotations

import os
import secrets
from dataclasses import dataclass
from pathlib import Path

from .archive import ArchiveLimits, extract_registry_archive
from .entry_paths import EntryName, parse_entry, validate_entry_set
from .errors import PackInstallError, PackInstallErrorCode
from .filesystem import remove_tree, sha256_file
from .json_records import (
    read_record,
    text_field,
    text_list_field,
    text_map_field,
    write_record,
)
from .pack_identity import parse_release_version, require_pack_identity

_STAGED = "staged"
_PARTIAL_PREFIX = "staging-"
_MANIFEST = "manifest.json"
_FILES = "files"
_SCHEDULED = "scheduled.json"
MISSING_FILE_DIGEST = ""


@dataclass(frozen=True, slots=True)
class StagedRelease:
    """Describe the release waiting in the staging area."""

    version: str
    from_version: str
    source: str
    archive_sha256: str
    entries: tuple[EntryName, ...]
    file_digests: dict[str, str]
    installed_digests: dict[str, str]
    files_root: Path


@dataclass(frozen=True, slots=True)
class StagingRequest:
    """Describe one downloaded archive to stage."""

    archive_path: Path
    pack_id: str
    version: str
    from_version: str
    source: str
    archive_sha256: str
    installed_digests: dict[str, str | None]


class StagedStore:
    """Own the staging area and the user's install choice under one updates root."""

    def __init__(
        self, updates_root: Path, limits: ArchiveLimits = ArchiveLimits()
    ) -> None:
        """Bind the store to the pack's private updates directory."""

        self._root = updates_root
        self._limits = limits

    def stage(self, request: StagingRequest) -> StagedRelease:
        """Extract, check and publish one release, replacing any older staged one."""

        self.clear_partials()
        self._root.mkdir(parents=True, exist_ok=True)
        partial = self._root / f"{_PARTIAL_PREFIX}{secrets.token_hex(6)}"
        try:
            extracted = extract_registry_archive(
                request.archive_path, partial / _FILES, self._limits
            )
            require_pack_identity(
                partial / _FILES,
                pack_id=request.pack_id,
                version=parse_release_version(request.version),
            )
            write_record(
                partial / _MANIFEST,
                {
                    "version": request.version,
                    "fromVersion": request.from_version,
                    "source": request.source,
                    "archiveSha256": request.archive_sha256,
                    "entries": [entry.text for entry in extracted.entries],
                    "fileDigests": extracted.file_digests,
                    "installedDigests": {
                        path: digest or MISSING_FILE_DIGEST
                        for path, digest in request.installed_digests.items()
                    },
                },
            )
            self.unschedule()
            remove_tree(self._root / _STAGED)
            os.replace(partial, self._root / _STAGED)
        finally:
            remove_tree(partial)
        release = self.load()
        if release is None:
            raise PackInstallError(
                PackInstallErrorCode.STAGED_CORRUPT, "The staged release vanished."
            )
        return release

    def load(self) -> StagedRelease | None:
        """Return the staged release described by its manifest, if any."""

        staged = self._root / _STAGED
        record = read_record(staged / _MANIFEST)
        if record is None:
            return None
        entries = tuple(
            parse_entry(text) for text in text_list_field(record, "entries")
        )
        validate_entry_set(entries)
        return StagedRelease(
            version=text_field(record, "version"),
            from_version=text_field(record, "fromVersion"),
            source=text_field(record, "source"),
            archive_sha256=text_field(record, "archiveSha256"),
            entries=entries,
            file_digests=text_map_field(record, "fileDigests"),
            installed_digests=text_map_field(record, "installedDigests"),
            files_root=staged / _FILES,
        )

    def verify(self, release: StagedRelease) -> None:
        """Prove every staged file still has the digest it was staged with."""

        for entry in release.entries:
            if entry.is_directory:
                continue
            staged_file = release.files_root.joinpath(*entry.path.split("/"))
            expected = release.file_digests.get(entry.path)
            if (
                expected is None
                or not staged_file.is_file()
                or sha256_file(staged_file) != expected
            ):
                raise PackInstallError(
                    PackInstallErrorCode.STAGED_CORRUPT,
                    f"The staged file {entry.path!r} changed after staging.",
                )

    def discard(self) -> None:
        """Remove the staged release and any install choice made for it."""

        self.unschedule()
        remove_tree(self._root / _STAGED)

    def clear_partials(self) -> None:
        """Remove staging directories an interrupted process left behind."""

        if not self._root.is_dir():
            return
        for child in self._root.iterdir():
            if child.name.startswith(_PARTIAL_PREFIX):
                remove_tree(child)

    def schedule(self, release: StagedRelease) -> None:
        """Record the user's choice to install this staged release at next start."""

        write_record(
            self._root / _SCHEDULED,
            {"version": release.version, "archiveSha256": release.archive_sha256},
        )

    def scheduled_for(self, release: StagedRelease | None) -> bool:
        """Return whether the user chose to install exactly this staged release."""

        record = read_record(self._root / _SCHEDULED)
        if record is None or release is None:
            return False
        return (
            record.get("version") == release.version
            and record.get("archiveSha256") == release.archive_sha256
        )

    def has_schedule(self) -> bool:
        """Return whether any install choice is recorded."""

        return (self._root / _SCHEDULED).exists()

    def unschedule(self) -> None:
        """Withdraw the user's install choice."""

        (self._root / _SCHEDULED).unlink(missing_ok=True)


__all__ = [
    "MISSING_FILE_DIGEST",
    "StagedRelease",
    "StagedStore",
    "StagingRequest",
]
