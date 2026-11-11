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
"""Commit fully prepared native image layers without changing target canvas geometry."""

from __future__ import annotations

from collections.abc import Callable

from .file_api import NativeImageFiles
from .images import resolve_document
from .model import (
    NativeDocument,
    NativeInsertedLayer,
    NativeOperationCancelled,
    NativeOperationUnsupported,
    NativePng,
)
from .output_api import GimpOutputApi, NativeOutputLayer


class NativeOutputs:
    """Own native decoding, precommit validation and unattached layer cleanup."""

    def __init__(self, gimp: GimpOutputApi, files: NativeImageFiles) -> None:
        """Borrow file lifetime while retaining every prepared native layer until commit or cleanup."""
        self._gimp = gimp
        self._files = files
        self._pending: list[NativeOutputLayer] = []
        self._closed = False

    def insert(
        self,
        document: NativeDocument,
        png: NativePng,
        name: str,
        left: int,
        top: int,
        cancelled: Callable[[], bool],
    ) -> NativeInsertedLayer:
        """Prepare off-image and use one native insertion as the mutation boundary.

        Args:
            document: Captured destination identity and geometry.
            png: Bytes and dimensions admitted by the shared PNG policy.
            name: Literal layer label.
            left: Origin computed by the shared placement owner.
            top: Origin computed by the shared placement owner.
            cancelled: Request cancellation supplied by the native transport.

        Raises:
            NativeOperationCancelled: Cancellation arrived before insertion.
            NativeOperationUnsupported: The destination is not an RGB image.
            RuntimeError: Native decoding, validation or cleanup failed.
        """
        if self._closed:
            raise RuntimeError("The native output session has closed.")
        if self._pending:
            raise RuntimeError("A prepared native layer still requires cleanup.")
        _check_cancelled(cancelled)
        target = resolve_document(self._gimp.Image, document)
        if target.get_base_type() != self._gimp.ImageBaseType.RGB:
            raise NativeOperationUnsupported(
                "GIMP output insertion requires an RGB image."
            )
        with self._files.png(png.data) as file:
            layer = self._gimp.file_load_layer(
                self._gimp.RunMode.NONINTERACTIVE, target, file
            )
            if layer is None:
                raise RuntimeError("GIMP could not decode the output PNG.")
            self._pending.append(layer)
            committed = False
            try:
                if (layer.get_width(), layer.get_height()) != (png.width, png.height):
                    raise RuntimeError(
                        "The decoded GIMP layer differs from its admitted PNG dimensions."
                    )
                if not layer.set_name(name) or not layer.set_offsets(left, top):
                    raise RuntimeError("GIMP could not prepare the output layer.")
                tattoo = layer.get_tattoo()
                if tattoo < 1:
                    raise RuntimeError("GIMP did not assign an output layer identity.")
                target = resolve_document(self._gimp.Image, document)
                _check_cancelled(cancelled)
                if not target.insert_layer(layer, None, 0):
                    raise RuntimeError("GIMP could not insert the output layer.")
                committed = True
                self._pending.remove(layer)
                return NativeInsertedLayer(tattoo, left, top, png.width, png.height)
            finally:
                if not committed:
                    self._release(layer)

    def close(self) -> None:
        """Retry owned native cleanup without touching layers already committed to user documents."""
        self._closed = True
        errors: list[Exception] = []
        for layer in tuple(self._pending):
            try:
                self._release(layer)
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("GIMP output preparation cleanup failed.", errors)

    def _release(self, layer: NativeOutputLayer) -> None:
        """Retain failed deletes so a later cleanup attempt remains possible."""
        if layer.is_valid() and not layer.delete():
            raise RuntimeError("GIMP could not release the prepared output layer.")
        self._pending.remove(layer)


def _check_cancelled(cancelled: Callable[[], bool]) -> None:
    """Stop before allocating or committing when a request has been cancelled."""
    if cancelled():
        raise NativeOperationCancelled("GIMP output insertion cancelled.")
