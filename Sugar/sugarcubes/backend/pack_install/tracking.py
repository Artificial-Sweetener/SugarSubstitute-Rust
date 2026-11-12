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
"""Read and write the Comfy Registry `.tracking` ownership record.

ComfyUI Manager writes the archive's member names joined by newlines in text mode,
so the record uses the platform line separator and has no trailing separator.
Readers accept LF or CRLF and ignore blank lines. The record names exactly the
paths the registry install owns; everything else in the folder is user data.
"""

from __future__ import annotations

import os
from pathlib import Path

from .entry_paths import TRACKING_FILE_NAME, EntryName, parse_entry, validate_entry_set
from .errors import PackInstallError, PackInstallErrorCode
from .filesystem import durable_replace_text

# .tracking carries no version of its own (ComfyUI Manager defines it); this is the
# version the shared install-format catalog records for the layout written here.
TRACKING_FORMAT_VERSION = 1
MAX_TRACKING_BYTES = 4 * 1024 * 1024
MAX_TRACKED_ENTRIES = 20_000


def parse_tracking(content: bytes) -> tuple[EntryName, ...]:
    """Return the validated entries of one record, in recorded order."""

    if len(content) > MAX_TRACKING_BYTES:
        raise PackInstallError(
            PackInstallErrorCode.TRACKING_TOO_LARGE,
            "The ownership record exceeds the size limit.",
        )
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise PackInstallError(
            PackInstallErrorCode.TRACKING_UNREADABLE,
            "The ownership record is not UTF-8 text.",
        ) from exc
    # ComfyUI Manager strips each line when it reads the record; do the same.
    entries = tuple(
        parse_entry(line.strip()) for line in text.split("\n") if line.strip()
    )
    if len(entries) > MAX_TRACKED_ENTRIES:
        raise PackInstallError(
            PackInstallErrorCode.TOO_MANY_ENTRIES,
            "The ownership record lists too many entries.",
        )
    validate_entry_set(entries)
    return entries


def render_tracking(entries: tuple[EntryName, ...]) -> str:
    """Return the logical record text, with LF separators before text-mode writing."""

    return "\n".join(entry.text for entry in entries)


def read_tracking(folder: Path) -> tuple[EntryName, ...] | None:
    """Return the folder's validated record, or `None` when it has none."""

    record = folder / TRACKING_FILE_NAME
    try:
        if record.is_symlink() or (record.exists() and not record.is_file()):
            raise PackInstallError(
                PackInstallErrorCode.TRACKING_UNREADABLE,
                "The ownership record is not a regular file.",
            )
        if not record.exists():
            return None
        with record.open("rb") as handle:
            content = handle.read(MAX_TRACKING_BYTES + 1)
    except OSError as exc:
        raise PackInstallError(
            PackInstallErrorCode.TRACKING_UNREADABLE,
            "The ownership record could not be read.",
        ) from exc
    return parse_tracking(content)


def write_tracking(folder: Path, entries: tuple[EntryName, ...]) -> None:
    """Durably replace the record with platform line separators, like Manager."""

    validate_entry_set(entries)
    durable_replace_text(
        folder / TRACKING_FILE_NAME,
        render_tracking(entries).replace("\n", os.linesep),
    )


__all__ = [
    "MAX_TRACKED_ENTRIES",
    "MAX_TRACKING_BYTES",
    "TRACKING_FORMAT_VERSION",
    "parse_tracking",
    "read_tracking",
    "render_tracking",
    "write_tracking",
]
