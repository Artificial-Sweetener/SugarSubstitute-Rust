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
"""Validate the relative entry names an archive and a `.tracking` record may hold.

An entry is a forward-slash relative path; a trailing slash marks a directory, as
in a zip listing. Every rule here is checked lexically, before any filesystem
access, and applies on every platform so a pack is portable to Windows.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from .errors import PackInstallError, PackInstallErrorCode

TRACKING_FILE_NAME = ".tracking"
MAX_ENTRY_LENGTH = 512
_WINDOWS_RESERVED_STEM = re.compile(
    r"^(?:con|prn|aux|nul|com[0-9¹²³]|lpt[0-9¹²³])$", re.IGNORECASE
)
_FORBIDDEN_CHARACTERS = frozenset('\\:*?"<>|')
_FORBIDDEN_SEGMENTS = frozenset({".git"})


@dataclass(frozen=True, slots=True)
class EntryName:
    """Carry one validated entry and whether it names a directory."""

    text: str
    is_directory: bool

    @property
    def path(self) -> str:
        """Return the entry without its directory marker."""

        return self.text.rstrip("/")

    @property
    def folded(self) -> str:
        """Return the case-folded path used to detect collisions on Windows."""

        return self.path.casefold()


def parse_entry(raw: str) -> EntryName:
    """Return one validated entry or raise a typed rejection.

    Rejected: empty names, absolute or drive paths, backslashes, alternate data
    streams, `.` and `..` segments, empty segments, control characters, segments
    ending in a dot or space, Windows device names, `.git` metadata and the
    ownership record itself.
    """

    if not raw or len(raw) > MAX_ENTRY_LENGTH or raw != raw.strip():
        raise _unsafe(raw)
    is_directory = raw.endswith("/")
    body = raw[:-1] if is_directory else raw
    if not body or body.startswith("/"):
        raise _unsafe(raw)
    segments = body.split("/")
    for segment in segments:
        _validate_segment(raw, segment)
    if not is_directory and body == TRACKING_FILE_NAME:
        raise PackInstallError(
            PackInstallErrorCode.RESERVED_NAME,
            "An archive may not carry its own ownership record.",
        )
    return EntryName(text=raw, is_directory=is_directory)


def _validate_segment(raw: str, segment: str) -> None:
    """Reject one path segment that is unsafe or unportable."""

    if segment in {"", ".", ".."}:
        raise _unsafe(raw)
    if any(
        ord(character) < 32 or character in _FORBIDDEN_CHARACTERS
        for character in segment
    ):
        raise _unsafe(raw)
    if segment.endswith((".", " ")):
        raise _unsafe(raw)
    if segment.casefold() in _FORBIDDEN_SEGMENTS:
        raise PackInstallError(
            PackInstallErrorCode.RESERVED_NAME,
            "An archive may not carry Git metadata.",
        )
    if _WINDOWS_RESERVED_STEM.fullmatch(segment.split(".", 1)[0]):
        raise PackInstallError(
            PackInstallErrorCode.RESERVED_NAME,
            "An entry uses a name Windows reserves for devices.",
        )


def _unsafe(raw: str) -> PackInstallError:
    """Build the rejection for an unsafe entry without echoing control bytes."""

    return PackInstallError(
        PackInstallErrorCode.UNSAFE_PATH,
        f"Unsafe entry name {raw.encode('unicode_escape').decode('ascii')[:120]!r}.",
    )


def validate_entry_set(entries: tuple[EntryName, ...]) -> None:
    """Reject duplicate or contradictory entries in one listing.

    Collisions are compared case-insensitively because the pack must install on a
    case-insensitive Windows volume. A file may not also be a directory or the
    parent of another entry.
    """

    files: set[str] = set()
    directories: set[str] = set()
    for entry in entries:
        key = entry.folded
        if key in files or (not entry.is_directory and key in directories):
            raise PackInstallError(
                PackInstallErrorCode.DUPLICATE_ENTRY,
                f"Entry {entry.path!r} appears more than once.",
            )
        if entry.is_directory:
            directories.add(key)
        else:
            files.add(key)
    for entry in entries:
        parts = entry.folded.split("/")
        for depth in range(1, len(parts)):
            if "/".join(parts[:depth]) in files:
                raise PackInstallError(
                    PackInstallErrorCode.ENTRY_CONFLICT,
                    f"Entry {entry.path!r} sits inside a file entry.",
                )


__all__ = [
    "EntryName",
    "MAX_ENTRY_LENGTH",
    "TRACKING_FILE_NAME",
    "parse_entry",
    "validate_entry_set",
]
