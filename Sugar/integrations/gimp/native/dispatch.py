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
"""Translate admitted private primitive requests into public GIMP SDK operations."""

from __future__ import annotations

from base64 import b64decode, b64encode
from collections.abc import Callable

from .contract import fields, integer, number, records, text
from .channels import NativeChannels
from .channel_editing import NativeChannelEditing
from .images import describe_image, resolve_document, resolve_image
from .layers import layer_catalog, primary_layer
from .model import (
    NativeDocument,
    NativeLayerInfo,
    NativeOperationCancelled,
    NativePixels,
    NativePng,
    StaleNativeImage,
)
from .outputs import NativeOutputs
from .sdk import NativeSdk
from .geometry import NativeGeometry
from .snapshots import NativeSnapshots, NativeVisibility


def _document(value: object) -> NativeDocument:
    """Convert admitted wire geometry into a typed native destination."""
    item = fields(value)
    return NativeDocument(
        integer(item["imageId"]),
        text(item["revision"]),
        integer(item["width"]),
        integer(item["height"]),
        text(item["name"]),
    )


def _describe(document: NativeDocument) -> dict[str, object]:
    """Return native metadata without exposing a GI object or applying workflow policy."""
    return {
        "imageId": document.image_id,
        "revision": document.revision,
        "width": document.width,
        "height": document.height,
        "name": _label(document.name),
    }


def _label(value: str) -> str:
    """Bound native labels in the companion's UTF-16 units without splitting an astral character."""
    return value.encode("utf-16-le")[:512].decode("utf-16-le", errors="ignore")


def _pixels(pixels: NativePixels) -> dict[str, object]:
    """Frame exact SDK bytes without resampling or changing coverage semantics."""
    return {
        "kind": "pixels",
        "width": pixels.width,
        "height": pixels.height,
        "format": pixels.format,
        "data": b64encode(pixels.data).decode("ascii"),
    }


class NativeDispatch:
    """Bind one launch image and retain snapshot ownership across cancelled pipe requests."""

    def __init__(
        self,
        sdk: NativeSdk,
        image_id: int,
        snapshots: NativeSnapshots,
        outputs: NativeOutputs,
        maximum_png_bytes: int,
        host_identity: str,
        channels: NativeChannels,
        editing: NativeChannelEditing,
    ) -> None:
        """Borrow cohesive native services and a shared admission budget from the package."""
        self._sdk = sdk
        self._image_id = image_id
        self._snapshots = snapshots
        self._outputs = outputs
        self._maximum_png_bytes = maximum_png_bytes
        self._host_identity = host_identity
        self._channels = channels
        self._editing = editing
        self._geometry = NativeGeometry(sdk.geometry)
        self._snapshot_requests: dict[int, str] = {}

    @property
    def retained_requests(self) -> frozenset[int]:
        """Keep cancellation addressable until an offscreen snapshot has actually been released."""
        return frozenset(self._snapshot_requests)

    def execute(
        self, request_id: int, command: dict[str, object], cancelled: Callable[[], bool]
    ) -> dict[str, object]:
        """Translate a validated primitive on the GIMP main loop without running a workflow."""
        if cancelled():
            raise NativeOperationCancelled("GIMP native request cancelled.")
        kind = command["type"]
        if kind == "geometry.context":
            document = self._destination(command["document"])
            selection = self._geometry.selection(document)
            return {
                "kind": "geometry",
                "document": _describe(document),
                "selection": (
                    None
                    if selection is None
                    else dict(
                        zip(("left", "top", "width", "height"), selection, strict=True)
                    )
                ),
            }
        if kind == "image.resize":
            document = self._geometry.resize(
                self._destination(command["document"]),
                integer(command["width"]),
                integer(command["height"]),
                cancelled,
            )
            return {
                "kind": "resized",
                "document": _describe(document),
                "committed": True,
            }
        if kind == "context":
            return self._context()
        if kind == "channel.create":
            source = command["source"]
            if source not in ("selection", "blank", "pixels"):
                raise ValueError("Unknown native channel source.")
            data = command["data"]
            tattoo = self._channels.create(
                self._destination(command["document"]),
                text(command["name"]),
                (
                    "selection"
                    if source == "selection"
                    else "blank" if source == "blank" else "pixels"
                ),
                None if data is None else b64decode(text(data), validate=True),
                cancelled,
            )
            return {"kind": "channel-created", "tattoo": tattoo, "committed": True}
        if kind == "channel.select":
            self._editing.select(
                self._destination(command["document"]), integer(command["tattoo"])
            )
            return {"kind": "channel-selected", "committed": True}
        if kind == "channel.release":
            self._editing.release(
                None if command["tattoo"] is None else integer(command["tattoo"])
            )
            return {"kind": "channel-selected", "committed": True}
        if kind == "snapshot.release":
            token = text(command["token"])
            self._snapshots.release(token)
            self._snapshot_requests = {
                key: item
                for key, item in self._snapshot_requests.items()
                if item != token
            }
            return {"kind": "released"}
        if kind == "snapshot.begin":
            expected = self._destination(command["document"])
            resolve_document(self._sdk.gimp.Image, expected)
            snapshot = self._snapshots.begin(expected.image_id, expected.revision)
            self._snapshot_requests[request_id] = snapshot.token
            if cancelled():
                self.cancel(request_id)
                raise NativeOperationCancelled("GIMP native capture cancelled.")
            return {
                "kind": "snapshot",
                "token": snapshot.token,
                "document": _describe(snapshot.document),
                "layers": self._layers(snapshot.layers),
                "primaryLayer": snapshot.primary_layer,
            }
        if kind == "image.insert":
            document = self._destination(command["document"])
            data = b64decode(text(command["data"]), validate=True)
            if not data or len(data) > self._maximum_png_bytes:
                raise ValueError("Native PNG data exceeds its admitted byte budget.")
            result = self._outputs.insert(
                document,
                NativePng(data, integer(command["width"]), integer(command["height"])),
                text(command["name"]),
                integer(command["left"]),
                integer(command["top"]),
                cancelled,
            )
            return {
                "kind": "inserted",
                "committed": True,
                "tattoo": result.tattoo,
                "left": result.left,
                "top": result.top,
                "width": result.width,
                "height": result.height,
            }
        token, width, height, scale = (
            text(command["token"]),
            integer(command["width"]),
            integer(command["height"]),
            number(command["scale"]),
        )
        if kind == "snapshot.selection":
            return _pixels(self._snapshots.selection(token, width, height, scale))
        if kind == "snapshot.channel":
            return _pixels(
                self._snapshots.channel(
                    token, integer(command["tattoo"]), width, height, scale
                )
            )
        if kind == "snapshot.projection":
            visibility = tuple(
                NativeVisibility(integer(item["tattoo"]), item["visible"] is True)
                for item in records(command["visibility"])
            )
            return _pixels(
                self._snapshots.projection(token, visibility, width, height, scale)
            )
        raise ValueError("Unknown native primitive.")

    def cancel(self, request_id: int) -> None:
        """Release a snapshot even if cancellation arrives after its response left the SDK callback."""
        token = self._snapshot_requests.get(request_id)
        if token is not None:
            self._snapshots.release(token)
            del self._snapshot_requests[request_id]

    def close(self) -> None:
        """Release native captures and prepared output layers without touching committed user layers."""
        errors: list[Exception] = []
        for service in (self._snapshots, self._outputs, self._channels, self._editing):
            try:
                service.close()
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("Native GIMP session cleanup failed.", errors)
        self._snapshot_requests.clear()

    def _destination(self, value: object) -> NativeDocument:
        """Refuse another document rather than following a later active image implicitly."""
        document = _document(value)
        if document.image_id != self._image_id:
            raise StaleNativeImage(
                "The GIMP document differs from this companion's launch image."
            )
        return document

    def _context(self) -> dict[str, object]:
        """Read only the explicit launch image and report closure without substituting another image."""
        try:
            image = resolve_image(self._sdk.gimp.Image, self._image_id)
        except StaleNativeImage:
            return {
                "kind": "context",
                "hostId": self._host_identity,
                "document": None,
                "layers": [],
                "primaryLayer": None,
                "rgb": False,
            }
        document = describe_image(image)
        layers, primary = layer_catalog(image), primary_layer(image)
        resolve_document(self._sdk.gimp.Image, document)
        return {
            "kind": "context",
            "hostId": self._host_identity,
            "document": _describe(document),
            "layers": self._layers(layers),
            "primaryLayer": primary,
            "rgb": image.get_base_type() == self._sdk.output.ImageBaseType.RGB,
        }

    @staticmethod
    def _layers(layers: tuple[NativeLayerInfo, ...]) -> list[dict[str, object]]:
        """Preserve native identities and order while bounding presentation labels."""
        return [
            {
                "tattoo": layer.tattoo,
                "parentTattoo": layer.parent_tattoo,
                "name": _label(layer.name),
                "depth": layer.depth,
                "visible": layer.visible,
                "hasMask": layer.has_mask,
            }
            for layer in layers
        ]
