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
"""Prepare and persist authored native coverage channels without retaining editing-surface policy."""

from __future__ import annotations

from collections.abc import Callable
from typing import Literal

from .channel_api import GimpChannelApi, NativeChannel
from .channel_identity import MASK_MARKER, MASK_VERSION
from .channel_editing import capture_channel_focus, restore_channel_focus
from .images import resolve_document
from .model import NativeDocument, NativeOperationCancelled
from .pixels import NativePixelReader
from .sdk import GeglApi


class NativeChannels:
    """Keep native mask preparation and its single insertion commit point under one owner."""

    def __init__(
        self,
        gimp: GimpChannelApi,
        gegl: GeglApi,
        maximum_bytes: int,
        report: Callable[[Exception], None],
    ) -> None:
        """Borrow native collaborators and the authoritative shared coverage budget."""
        self._gimp = gimp
        self._gegl = gegl
        self._pixels = NativePixelReader(gegl, maximum_bytes)
        self._report = report
        self._pending: list[NativeChannel] = []
        self._closed = False

    def create(
        self,
        document: NativeDocument,
        name: str,
        source: Literal["selection", "blank", "pixels"],
        data: bytes | None,
        cancelled: Callable[[], bool],
    ) -> int:
        """Prepare full-canvas coverage off-image, then commit exactly one undoable channel insertion."""
        if self._closed or self._pending:
            raise RuntimeError(
                "The native channel session is closed or still requires cleanup."
            )
        self._check_cancelled(cancelled)
        image = resolve_document(self._gimp.Image, document)
        size = self._pixels.admit(
            document, document.width, document.height, 1, "coverage8"
        )
        if source == "selection":
            data = self._pixels.read(
                image.get_selection(),
                document,
                document.width,
                document.height,
                1,
                "coverage8",
            ).data
        elif source == "blank":
            data = bytes(size)
        if data is None or len(data) != size:
            raise ValueError("The authored mask and GIMP document dimensions differ.")
        channel = self._gimp.Channel.new(
            image,
            name,
            document.width,
            document.height,
            50,
            self._gegl.Color.new("red"),
        )
        if channel is None:
            raise RuntimeError("GIMP could not prepare the authored coverage channel.")
        self._pending.append(channel)
        committed = False
        try:
            buffer = channel.get_buffer()
            buffer.set(
                self._gegl.Rectangle.new(0, 0, document.width, document.height),
                (
                    "Y' u8"
                    if image.get_precision() == self._gimp.Precision.U8_NON_LINEAR
                    else "Y u8"
                ),
                data,
            )
            buffer.flush()
            marker = self._gimp.Parasite.new(
                MASK_MARKER, self._gimp.PARASITE_PERSISTENT, MASK_VERSION
            )
            if (
                not channel.update(0, 0, document.width, document.height)
                or not channel.attach_parasite(marker)
                or not channel.set_visible(False)
            ):
                raise RuntimeError(
                    "GIMP could not prepare the authored channel metadata."
                )
            tattoo = channel.get_tattoo()
            if tattoo < 1 or tattoo > 4294967295:
                raise RuntimeError("GIMP did not assign an authored channel identity.")
            image = resolve_document(self._gimp.Image, document)
            self._check_cancelled(cancelled)
            focus = capture_channel_focus(image)
            if not image.insert_channel(channel, None, 0):
                raise RuntimeError(
                    "GIMP could not insert the authored coverage channel."
                )
            committed = True
            self._pending.remove(channel)
            try:
                if tuple(
                    item.get_tattoo() for item in image.get_selected_channels()
                ) == (tattoo,) and not restore_channel_focus(image, focus):
                    raise RuntimeError(
                        "GIMP could not restore native focus after creating the saved mask."
                    )
            except Exception as error:
                self._report(error)
            return tattoo
        finally:
            if not committed:
                self._release_pending(channel)

    def close(self) -> None:
        """Release uncommitted resources while retaining authored channels in the native image."""
        self._closed = True
        errors: list[Exception] = []
        for channel in tuple(self._pending):
            try:
                self._release_pending(channel)
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("GIMP authored channel cleanup failed.", errors)

    def _release_pending(self, channel: NativeChannel) -> None:
        """Retain a failed native release so shutdown can retry it."""
        if channel.is_valid() and not channel.delete():
            raise RuntimeError("GIMP could not release the prepared authored channel.")
        self._pending.remove(channel)

    @staticmethod
    def _check_cancelled(cancelled: Callable[[], bool]) -> None:
        """Honor cancellation before native admission and the single insertion commit point."""
        if cancelled():
            raise NativeOperationCancelled("GIMP authored channel creation cancelled.")
