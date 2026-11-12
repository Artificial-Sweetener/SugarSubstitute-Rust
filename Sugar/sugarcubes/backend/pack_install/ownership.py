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
"""Decide whether an installed pack folder is a registry install we may replace.

Only a folder that is itself a plain directory, is not a Git checkout, and holds a
valid `.tracking` record is owned. Even then, a tracked file whose bytes differ
from the published release of the installed version is a local edit, and the
folder is left alone. Failing closed here is what keeps user work safe.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from .entry_paths import EntryName
from .errors import PackInstallError
from .filesystem import is_link_or_reparse_point, sha256_file
from .pack_identity import PackIdentity, read_pack_identity
from .tracking import read_tracking


class NotOwnedReason(str, Enum):
    """Name why a pack folder is left alone; spelled as in the shared vectors."""

    MISSING = "missing"
    LINKED_FOLDER = "linked-folder"
    GIT_CHECKOUT = "git-checkout"
    NOT_REGISTRY_INSTALL = "not-registry-install"
    INVALID_TRACKING = "invalid-tracking"
    LINKED_CONTENT = "linked-content"
    CHANGED_FILES = "changed-files"


@dataclass(frozen=True, slots=True)
class OwnedPack:
    """Describe a registry install this updater may replace."""

    folder: Path
    entries: tuple[EntryName, ...]
    identity: PackIdentity

    @property
    def file_paths(self) -> tuple[str, ...]:
        """Return the tracked file paths, in recorded order."""

        return tuple(entry.path for entry in self.entries if not entry.is_directory)


@dataclass(frozen=True, slots=True)
class NotOwnedPack:
    """Describe a pack folder this updater must not touch, and why."""

    folder: Path
    reason: NotOwnedReason
    detail: str


PackOwnership = OwnedPack | NotOwnedPack


def inspect_pack_folder(folder: Path) -> PackOwnership:
    """Classify one installed pack folder without modifying it.

    `folder` must be the path ComfyUI loads the pack from, not a resolved path:
    resolving would hide a junction or symbolic link.
    """

    if is_link_or_reparse_point(folder):
        return NotOwnedPack(
            folder, NotOwnedReason.LINKED_FOLDER, "The folder is a link."
        )
    if not folder.is_dir():
        return NotOwnedPack(
            folder, NotOwnedReason.MISSING, "The folder does not exist."
        )
    if (folder / ".git").exists() or (folder / ".git").is_symlink():
        return NotOwnedPack(
            folder, NotOwnedReason.GIT_CHECKOUT, "The folder is a Git checkout."
        )
    try:
        entries = read_tracking(folder)
    except PackInstallError as exc:
        return NotOwnedPack(folder, NotOwnedReason.INVALID_TRACKING, exc.detail)
    if entries is None:
        return NotOwnedPack(
            folder, NotOwnedReason.NOT_REGISTRY_INSTALL, "The folder has no .tracking."
        )
    try:
        identity = read_pack_identity(folder)
    except PackInstallError as exc:
        return NotOwnedPack(folder, NotOwnedReason.NOT_REGISTRY_INSTALL, exc.detail)
    linked = _first_linked_entry(folder, entries)
    if linked is not None:
        return NotOwnedPack(
            folder,
            NotOwnedReason.LINKED_CONTENT,
            f"Tracked entry {linked!r} is a link or has an unexpected type.",
        )
    return OwnedPack(folder=folder, entries=entries, identity=identity)


def _first_linked_entry(folder: Path, entries: tuple[EntryName, ...]) -> str | None:
    """Return the first tracked entry reached through a link or of the wrong type."""

    for entry in entries:
        current = folder
        for segment in entry.path.split("/"):
            current = current / segment
            if is_link_or_reparse_point(current):
                return entry.path
        if not current.exists():
            continue
        if entry.is_directory != current.is_dir():
            return entry.path
    return None


def installed_file_digests(pack: OwnedPack) -> dict[str, str | None]:
    """Return each tracked file's SHA-256, or `None` for a tracked file now absent."""

    digests: dict[str, str | None] = {}
    for path in pack.file_paths:
        installed = pack.folder.joinpath(*path.split("/"))
        digests[path] = sha256_file(installed) if installed.is_file() else None
    return digests


def changed_tracked_files(
    installed: Mapping[str, str | None], published: Mapping[str, str]
) -> tuple[str, ...]:
    """Return tracked files whose content is not the published release's.

    A tracked file the release does not contain is also a change, because nothing
    proves it came from that release. A tracked file the user deleted holds no
    work to lose, so it is not a change.
    """

    return tuple(
        path
        for path, digest in installed.items()
        if digest is not None and published.get(path) != digest
    )


__all__ = [
    "NotOwnedPack",
    "NotOwnedReason",
    "OwnedPack",
    "PackOwnership",
    "changed_tracked_files",
    "inspect_pack_folder",
    "installed_file_digests",
]
