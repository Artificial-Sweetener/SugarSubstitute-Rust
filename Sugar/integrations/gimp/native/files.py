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
"""Materialize admitted PNG data and retain ownership of failed temporary-file cleanup."""

from __future__ import annotations

from collections.abc import Callable, Iterator
from contextlib import contextmanager
from pathlib import Path
from tempfile import NamedTemporaryFile

from .file_api import GioFileApi, NativeFile


class NativePngFiles:
    """Own decoder input files independently of native layer mutation."""

    def __init__(
        self,
        gio: GioFileApi,
        maximum_bytes: int,
        report: Callable[[Exception], None],
        maximum_live: int = 4,
    ) -> None:
        """Receive the shared transfer budget and a native diagnostics boundary."""
        if maximum_bytes < 1 or maximum_live < 1:
            raise ValueError("Native file limits must be positive.")
        self._gio = gio
        self._maximum_bytes = maximum_bytes
        self._maximum_live = maximum_live
        self._report = report
        self._paths: set[Path] = set()
        self._closed = False

    @contextmanager
    def png(self, data: bytes) -> Iterator[NativeFile]:
        """Materialize only admitted bytes and keep post-commit cleanup faults observable."""
        if self._closed:
            raise RuntimeError("The native image-file session has closed.")
        if not data or len(data) > self._maximum_bytes:
            raise ValueError("The native PNG exceeds its admitted byte budget.")
        if len(self._paths) >= self._maximum_live:
            raise RuntimeError("Native image files still require cleanup.")
        path: Path | None = None
        try:
            with NamedTemporaryFile(
                prefix="sugarcubes-gimp-", suffix=".png", delete=False
            ) as file:
                path = Path(file.name)
                self._paths.add(path)
                file.write(data)
            yield self._gio.File.new_for_path(str(path))
        finally:
            if path is not None:
                try:
                    self._release(path)
                except OSError as error:
                    self._report(error)

    def close(self) -> None:
        """Retry every owned path without permitting new file allocations."""
        self._closed = True
        errors: list[Exception] = []
        for path in tuple(self._paths):
            try:
                self._release(path)
            except OSError as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("GIMP temporary input cleanup failed.", errors)

    def _release(self, path: Path) -> None:
        """Forget a path only after its native decoder input has been removed."""
        path.unlink(missing_ok=True)
        self._paths.discard(path)
