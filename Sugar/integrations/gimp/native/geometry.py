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
"""Apply geometry primitives to one captured native document without workflow policy."""

from __future__ import annotations

from collections.abc import Callable

from .geometry_api import GimpGeometryApi
from .images import describe_image, resolve_document
from .model import NativeDocument, NativeOperationCancelled


class NativeGeometry:
    """Own exact native canvas reads and a single undoable bounds mutation."""

    def __init__(self, api: GimpGeometryApi) -> None:
        """Borrow only the installed geometry API, independent of the companion viewer."""
        self._api = api

    def selection(self, expected: NativeDocument) -> tuple[int, int, int, int] | None:
        """Read coherent selection extents in the captured document coordinate space."""
        image = resolve_document(self._api.Image, expected)
        success, nonempty, left, top, right, bottom = self._api.Selection.bounds(image)
        if not success:
            raise RuntimeError("GIMP could not read the document selection bounds.")
        resolve_document(self._api.Image, expected)
        if not nonempty:
            return None
        if not (
            0 <= left < right <= expected.width and 0 <= top < bottom <= expected.height
        ):
            raise RuntimeError(
                "GIMP returned selection bounds outside the captured canvas."
            )
        return left, top, right - left, bottom - top

    def resize(
        self,
        expected: NativeDocument,
        width: int,
        height: int,
        cancelled: Callable[[], bool],
    ) -> NativeDocument:
        """Commit one centered bounds resize while preserving native pixels and undo.

        GIMP's resize procedure repositions layers and resizes channels, including
        selection coverage, as one native operation. No temporary document or
        separate selection mutation is necessary.

        Args:
            expected: Captured native identity and coordinate frame.
            width: Width admitted by the compiled shared canvas schema.
            height: Height admitted by the compiled shared canvas schema.
            cancelled: Request cancellation checked immediately before mutation.

        Returns:
            Fresh document metadata after the native operation completes.

        Raises:
            NativeOperationCancelled: Cancellation arrived before mutation.
            RuntimeError: Native resizing or postcommit verification failed.
        """
        image = resolve_document(self._api.Image, expected)
        if cancelled():
            raise NativeOperationCancelled("GIMP canvas sizing cancelled.")
        if (width, height) == (expected.width, expected.height):
            return describe_image(image)
        if not self._api.Image.resize(
            image,
            width,
            height,
            (width - expected.width) // 2,
            (height - expected.height) // 2,
        ):
            raise RuntimeError("GIMP could not resize the captured canvas.")
        document = describe_image(image)
        if (document.width, document.height) != (width, height):
            raise RuntimeError("GIMP did not apply the requested canvas dimensions.")
        return document
