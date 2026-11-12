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
"""Validate and extract a flat registry-layout pack archive.

A registry archive is a zip whose members sit at the pack root, as `comfy node
pack` produces and the Comfy Registry serves. The member listing, in archive
order, becomes the `.tracking` record. Member headers are not trusted: sizes are
counted while reading, and every name is validated before anything is written.
"""

from __future__ import annotations

import stat
import zipfile
from dataclasses import dataclass
from pathlib import Path

from .entry_paths import EntryName, parse_entry, validate_entry_set
from .errors import PackInstallError, PackInstallErrorCode
from .filesystem import sha256_file

_ENCRYPTED_FLAG = 0x1
_READ_CHUNK_BYTES = 1024 * 1024


@dataclass(frozen=True, slots=True)
class ArchiveLimits:
    """Bound what one archive may hold before extraction is refused."""

    max_entries: int = 20_000
    max_total_bytes: int = 256 * 1024 * 1024
    max_file_bytes: int = 64 * 1024 * 1024


@dataclass(frozen=True, slots=True)
class ExtractedArchive:
    """Describe a validated extraction: its entries and each file's digest."""

    entries: tuple[EntryName, ...]
    file_digests: dict[str, str]


def list_registry_archive(
    archive_path: Path, limits: ArchiveLimits = ArchiveLimits()
) -> tuple[EntryName, ...]:
    """Return the validated member entries of an archive without extracting it."""

    with _open_zip(archive_path) as archive:
        return _validated_entries(archive, limits)


def extract_registry_archive(
    archive_path: Path,
    target: Path,
    limits: ArchiveLimits = ArchiveLimits(),
) -> ExtractedArchive:
    """Extract a validated archive into an empty directory this caller owns."""

    target.mkdir(parents=True, exist_ok=False)
    total_bytes = 0
    digests: dict[str, str] = {}
    with _open_zip(archive_path) as archive:
        entries = _validated_entries(archive, limits)
        members = {info.filename: info for info in archive.infolist() if info.filename}
        for entry in entries:
            destination = target.joinpath(*entry.path.split("/"))
            if entry.is_directory:
                destination.mkdir(parents=True, exist_ok=True)
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            total_bytes = _copy_member(
                archive, members[entry.text], destination, total_bytes, limits
            )
            digests[entry.path] = sha256_file(destination)
    return ExtractedArchive(entries=entries, file_digests=digests)


def _open_zip(archive_path: Path) -> zipfile.ZipFile:
    """Open one archive, translating corrupt containers into a typed failure."""

    try:
        return zipfile.ZipFile(archive_path)
    except (zipfile.BadZipFile, OSError, EOFError) as exc:
        raise PackInstallError(
            PackInstallErrorCode.NOT_A_ZIP, "The download is not a readable zip."
        ) from exc


def _validated_entries(
    archive: zipfile.ZipFile, limits: ArchiveLimits
) -> tuple[EntryName, ...]:
    """Validate every member's name, type and declared size."""

    infos = [info for info in archive.infolist() if info.filename]
    if not infos:
        raise PackInstallError(
            PackInstallErrorCode.EMPTY_ARCHIVE, "The archive holds no files."
        )
    if len(infos) > limits.max_entries:
        raise PackInstallError(
            PackInstallErrorCode.TOO_MANY_ENTRIES, "The archive has too many members."
        )
    declared_total = 0
    entries: list[EntryName] = []
    for info in infos:
        entry = parse_entry(info.filename)
        _validate_member_kind(info, entry)
        declared_total += info.file_size
        if (
            info.file_size > limits.max_file_bytes
            or declared_total > limits.max_total_bytes
        ):
            raise PackInstallError(
                PackInstallErrorCode.TOO_LARGE, "The archive expands beyond the limit."
            )
        entries.append(entry)
    result = tuple(entries)
    validate_entry_set(result)
    if not any(not entry.is_directory for entry in result):
        raise PackInstallError(
            PackInstallErrorCode.EMPTY_ARCHIVE, "The archive holds no files."
        )
    return result


def _validate_member_kind(info: zipfile.ZipInfo, entry: EntryName) -> None:
    """Accept only plain files and directories, never links or encrypted data."""

    if info.flag_bits & _ENCRYPTED_FLAG:
        raise PackInstallError(
            PackInstallErrorCode.ENCRYPTED_MEMBER, "The archive has encrypted members."
        )
    mode = info.external_attr >> 16
    kind = stat.S_IFMT(mode)
    if stat.S_ISLNK(mode) or kind not in {0, stat.S_IFREG, stat.S_IFDIR}:
        raise PackInstallError(
            PackInstallErrorCode.LINK_MEMBER, "The archive has link or device members."
        )
    if entry.is_directory != (info.is_dir() or kind == stat.S_IFDIR):
        raise PackInstallError(
            PackInstallErrorCode.LINK_MEMBER,
            "An archive member's type does not match its name.",
        )


def _copy_member(
    archive: zipfile.ZipFile,
    info: zipfile.ZipInfo,
    destination: Path,
    total_bytes: int,
    limits: ArchiveLimits,
) -> int:
    """Copy one member while counting the bytes actually produced."""

    written = 0
    try:
        with archive.open(info) as source, destination.open("xb") as output:
            while chunk := source.read(_READ_CHUNK_BYTES):
                written += len(chunk)
                total_bytes += len(chunk)
                if (
                    written > limits.max_file_bytes
                    or total_bytes > limits.max_total_bytes
                ):
                    raise PackInstallError(
                        PackInstallErrorCode.TOO_LARGE,
                        "The archive expands beyond the limit.",
                    )
                output.write(chunk)
    except (zipfile.BadZipFile, EOFError, NotImplementedError) as exc:
        raise PackInstallError(
            PackInstallErrorCode.NOT_A_ZIP, "An archive member is corrupt."
        ) from exc
    return total_bytes


__all__ = [
    "ArchiveLimits",
    "ExtractedArchive",
    "extract_registry_archive",
    "list_registry_archive",
]
