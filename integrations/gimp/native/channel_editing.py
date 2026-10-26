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
"""Own the modeless native mask editing surface independently of persisted channel creation."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from .channel_api import GimpChannelApi, NativeChannel, NativeChannelImage
from .channel_identity import is_authored_channel
from .images import resolve_document
from .model import NativeDocument, StaleNativeImage


@dataclass(frozen=True, slots=True)
class NativeChannelFocus:
    """Retain the exact native editing surface around an automatic host selection change."""

    layers: tuple[int, ...]
    channels: tuple[int, ...]


def capture_channel_focus(image: NativeChannelImage) -> NativeChannelFocus:
    """Read native layer and channel identities without persisting editable labels."""
    return NativeChannelFocus(
        tuple(layer.get_tattoo() for layer in image.get_selected_layers()),
        tuple(channel.get_tattoo() for channel in image.get_selected_channels()),
    )


def restore_channel_focus(image: NativeChannelImage, focus: NativeChannelFocus) -> bool:
    """Restore still-valid native items without substituting another layer or channel."""
    channels = [
        channel
        for tattoo in focus.channels
        if (channel := image.get_channel_by_tattoo(tattoo)) is not None
        and channel.is_valid()
    ]
    layers = [
        layer
        for tattoo in focus.layers
        if (layer := image.get_layer_by_tattoo(tattoo)) is not None and layer.is_valid()
    ]
    return (
        image.set_selected_channels(channels)
        if channels
        else image.set_selected_layers(layers)
    )


@dataclass(frozen=True, slots=True)
class _Editing:
    """Retain only the native surface changed by this editor lifetime."""

    document: NativeDocument
    tattoo: int
    visible: bool
    layers: tuple[int, ...]
    channels: tuple[int, ...]


class NativeChannelEditing:
    """Restore only native focus and overlays owned by this companion lifetime."""

    def __init__(
        self, gimp: GimpChannelApi, report: Callable[[Exception], None]
    ) -> None:
        """Borrow native editing services without retaining any mask pixel storage."""
        self._gimp = gimp
        self._report = report
        self._active: _Editing | None = None
        self._closed = False

    def select(self, document: NativeDocument, tattoo: int) -> None:
        """Let GIMP's normal paint tools edit the saved mask without borrowing the active selection."""
        if self._closed:
            raise RuntimeError("The native channel session has closed.")
        image = resolve_document(self._gimp.Image, document)
        channel = self._channel(image, tattoo)
        if (
            self._active
            and self._active.document.image_id == document.image_id
            and self._active.tattoo == tattoo
            and tuple(item.get_tattoo() for item in image.get_selected_channels())
            == (tattoo,)
            and channel.get_visible()
        ):
            return
        self.release(None)
        image = resolve_document(self._gimp.Image, document)
        focus = capture_channel_focus(image)
        self._active = _Editing(
            document,
            tattoo,
            channel.get_visible(),
            focus.layers,
            focus.channels,
        )
        try:
            if not channel.set_visible(True) or not image.set_selected_channels(
                [channel]
            ):
                raise RuntimeError("GIMP could not activate the authored mask channel.")
            self._flush()
        except Exception:
            self.release(None)
            raise

    def release(self, tattoo: int | None) -> None:
        """Restore prior native focus without overriding a later deliberate layer or channel choice."""
        active = self._active
        if active is None or (tattoo is not None and tattoo != active.tattoo):
            return
        image = self._gimp.Image.get_by_id(active.document.image_id)
        if image is None or not image.is_valid():
            self._active = None
            return
        channel = image.get_channel_by_tattoo(active.tattoo)
        if (
            channel is not None
            and channel.is_valid()
            and not channel.set_visible(active.visible)
        ):
            raise RuntimeError("GIMP could not restore the authored channel overlay.")
        selected = tuple(item.get_tattoo() for item in image.get_selected_channels())
        if selected == (active.tattoo,):
            restored = restore_channel_focus(
                image, NativeChannelFocus(active.layers, active.channels)
            )
            if not restored:
                raise RuntimeError(
                    "GIMP could not restore the previous native editing surface."
                )
        self._active = None
        self._flush()

    def close(self) -> None:
        """Retire the owned editing surface without deleting authored channels."""
        self._closed = True
        self.release(None)

    def _flush(self) -> None:
        """Keep a failed redraw observable without denying an already committed native change."""
        if not self._gimp.displays_flush():
            self._report(
                RuntimeError("GIMP could not refresh its authored mask display.")
            )

    @staticmethod
    def _channel(image: NativeChannelImage, tattoo: int) -> NativeChannel:
        """Require persistent authored identity instead of an editable channel label."""
        channel = image.get_channel_by_tattoo(tattoo)
        if (
            channel is None
            or not channel.is_valid()
            or not is_authored_channel(channel)
        ):
            raise StaleNativeImage("The authored GIMP mask channel is unavailable.")
        return channel
