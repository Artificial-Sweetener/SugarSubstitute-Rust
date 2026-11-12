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
"""Define the typed failures of the registry install format.

The codes are shared with every implementation of the format (the Rust host reads
the same test vectors), so their spelling is part of the format contract.
"""

from __future__ import annotations

from enum import Enum


class PackInstallErrorCode(str, Enum):
    """Name one reason the install format rejects an archive, folder or record."""

    NOT_A_ZIP = "not-a-zip"
    EMPTY_ARCHIVE = "empty-archive"
    ENCRYPTED_MEMBER = "encrypted-member"
    LINK_MEMBER = "link-member"
    UNSAFE_PATH = "unsafe-path"
    RESERVED_NAME = "reserved-name"
    DUPLICATE_ENTRY = "duplicate-entry"
    ENTRY_CONFLICT = "entry-conflict"
    TOO_MANY_ENTRIES = "too-many-entries"
    TOO_LARGE = "too-large"
    TRACKING_UNREADABLE = "tracking-unreadable"
    TRACKING_TOO_LARGE = "tracking-too-large"
    PACK_IDENTITY = "pack-identity"
    STAGED_CORRUPT = "staged-corrupt"
    TARGET_CHANGED = "target-changed"
    BUSY = "busy"


class PackInstallError(RuntimeError):
    """Report a rejected archive, ownership record or install transaction."""

    def __init__(self, code: PackInstallErrorCode, detail: str) -> None:
        """Keep the typed code beside a diagnostic detail without secrets."""

        super().__init__(f"{code.value}: {detail}")
        self.code = code
        self.detail = detail


__all__ = ["PackInstallError", "PackInstallErrorCode"]
