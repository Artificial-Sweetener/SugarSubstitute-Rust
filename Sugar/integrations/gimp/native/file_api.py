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
"""Describe file handles passed to native image decoders."""

from __future__ import annotations

from contextlib import AbstractContextManager
from typing import Protocol


class NativeFile(Protocol):
    """Expose a native Gio file without coupling callers to dynamic GI types."""

    def get_path(self) -> str | None:
        """Read the local native file path for diagnostics and qualification."""
        ...


class NativeFileFactory(Protocol):
    """Create file handles only from adapter-owned local paths."""

    def new_for_path(self, path: str) -> NativeFile:
        """Wrap a materialized native input file."""
        ...


class GioFileApi(Protocol):
    """Expose installed Gio file construction."""

    File: NativeFileFactory


class NativeImageFiles(Protocol):
    """Borrow a temporary admitted PNG without claiming its filesystem lifetime."""

    def png(self, data: bytes) -> AbstractContextManager[NativeFile]:
        """Keep one native file alive only while the decoder needs it."""
        ...
