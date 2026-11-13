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
"""Own offscreen native image snapshots and release their temporary resources."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from uuid import uuid4

from .image_api import GimpImageApi, NativeImage, NativeLayer
from .images import describe_image, resolve_image
from .channel_identity import is_authored_channel
from .snapshot_identity import (
    NativeSnapshotBindings,
    bind_snapshot_items,
    native_item_catalog,
)
from .snapshot_presentation import prepare_snapshot_projection
from .model import NativeDocument, NativeLayerInfo, NativePixels, StaleNativeImage
from .pixels import NativePixelReader


@dataclass(frozen=True, slots=True)
class NativeVisibility:
    """Apply a visibility decision already resolved by the shared source adapter."""

    tattoo: int
    visible: bool


@dataclass(frozen=True, slots=True)
class NativeSnapshotInfo:
    """Identify one coherent native batch without exposing SDK objects to its caller."""

    token: str
    document: NativeDocument
    layers: tuple[NativeLayerInfo, ...]
    primary_layer: int | None


@dataclass(slots=True)
class _Snapshot:
    """Retain one undisplayed image until the batch's owner releases it."""

    image: NativeImage
    info: NativeSnapshotInfo
    bindings: NativeSnapshotBindings
    projections: list[NativeLayer] = field(default_factory=list)
    retired: bool = False


class NativeSnapshots:
    """Freeze native contents once per batch while leaving the original image editable."""

    def __init__(
        self, gimp: GimpImageApi, pixels: NativePixelReader, maximum_live: int = 4
    ) -> None:
        """Bound retained native images independently of capture scheduling."""
        if maximum_live < 1:
            raise ValueError("The live native snapshot limit must be positive.")
        self._gimp = gimp
        self._pixels = pixels
        self._maximum_live = maximum_live
        self._snapshots: dict[str, _Snapshot] = {}
        self._unadmitted: list[NativeImage] = []
        self._closed = False

    def begin(self, image_id: int, revision: str) -> NativeSnapshotInfo:
        """Duplicate the explicitly captured image and reject any geometry change during freezing."""
        if self._closed:
            raise StaleNativeImage("The native snapshot session has closed.")
        if len(self._snapshots) + len(self._unadmitted) >= self._maximum_live:
            raise RuntimeError("The native snapshot limit has been reached.")
        source = resolve_image(self._gimp.Image, image_id, revision)
        document = describe_image(source)
        if document.revision != revision:
            raise StaleNativeImage(
                "The GIMP canvas changed before its contents were frozen."
            )
        catalog = native_item_catalog(source)
        frozen = source.duplicate()
        self._unadmitted.append(frozen)
        retained = False
        try:
            if describe_image(frozen).revision != document.revision:
                raise StaleNativeImage(
                    "The GIMP canvas changed while its contents were frozen."
                )
            bindings = bind_snapshot_items(
                catalog, native_item_catalog(source), native_item_catalog(frozen)
            )
            prepare_snapshot_projection(self._gimp, frozen)
            info = NativeSnapshotInfo(
                str(uuid4()), document, catalog.layers, catalog.primary
            )
            self._snapshots[info.token] = _Snapshot(frozen, info, bindings)
            self._unadmitted.remove(frozen)
            retained = True
            return info
        finally:
            if not retained:
                _delete_image(frozen)
                self._unadmitted.remove(frozen)

    def selection(
        self, token: str, width: int, height: int, scale: float
    ) -> NativePixels:
        """Read frozen full-canvas coverage without changing the original selection."""
        snapshot = self._get(token)
        return self._pixels.read(
            snapshot.image.get_selection(),
            snapshot.info.document,
            width,
            height,
            scale,
            "coverage8",
        )

    def projection(
        self,
        token: str,
        visibility: Sequence[NativeVisibility],
        width: int,
        height: int,
        scale: float,
    ) -> NativePixels:
        """Render the resolved stack without changing the source document.

        Args:
            token: Live native batch identity.
            visibility: Overrides computed by the shared layer-range owner.
            width: Admitted output width in scaled coordinates.
            height: Admitted output height in scaled coordinates.
            scale: Uniform downsample factor selected by the shared adapter.

        Raises:
            StaleNativeImage: The batch or a requested native layer is unavailable.
            ExceptionGroup: Native cleanup failed; the batch must be released.
        """
        snapshot = self._get(token)
        self._pixels.admit(snapshot.info.document, width, height, scale, "rgba8")
        layers: list[tuple[NativeLayer, bool, bool]] = []
        seen: set[int] = set()
        for change in visibility:
            frozen_tattoo = snapshot.bindings.layers.get(change.tattoo)
            layer = (
                None
                if frozen_tattoo is None
                else snapshot.image.get_layer_by_tattoo(frozen_tattoo)
            )
            if layer is None or change.tattoo in seen:
                raise StaleNativeImage(
                    "The requested GIMP snapshot layer is unavailable or repeated."
                )
            seen.add(change.tattoo)
            layers.append((layer, layer.get_visible(), change.visible))
        changed: list[tuple[NativeLayer, bool]] = []
        try:
            for layer, previous, visible in layers:
                if previous == visible:
                    continue
                changed.append((layer, previous))
                if not layer.set_visible(visible):
                    raise RuntimeError("GIMP could not prepare the frozen projection.")
            projection = self._gimp.Layer.new_from_visible(
                snapshot.image, snapshot.image, "SugarCubes capture"
            )
            if projection is None:
                raise RuntimeError("GIMP could not create the frozen projection.")
            snapshot.projections.append(projection)
            try:
                return self._pixels.read(
                    projection, snapshot.info.document, width, height, scale, "rgba8"
                )
            finally:
                try:
                    _release_projection(snapshot, projection)
                except Exception:
                    snapshot.retired = True
                    raise
        finally:
            errors: list[Exception] = []
            for layer, previous in reversed(changed):
                try:
                    if not layer.set_visible(previous):
                        raise RuntimeError(
                            "GIMP could not restore frozen layer visibility."
                        )
                except Exception as error:
                    errors.append(error)
            if errors:
                snapshot.retired = True
                raise ExceptionGroup("GIMP snapshot visibility cleanup failed.", errors)

    def channel(
        self, token: str, tattoo: int, width: int, height: int, scale: float
    ) -> NativePixels:
        """Read persisted authored coverage from the same frozen image as every mask in this batch."""
        snapshot = self._get(token)
        frozen_tattoo = snapshot.bindings.channels.get(tattoo)
        channel = (
            None
            if frozen_tattoo is None
            else snapshot.image.get_channel_by_tattoo(frozen_tattoo)
        )
        if channel is None or not is_authored_channel(channel):
            raise StaleNativeImage("The authored GIMP mask channel is unavailable.")
        return self._pixels.read_channel(
            channel,
            snapshot.info.document,
            width,
            height,
            scale,
            snapshot.image.get_precision() == self._gimp.Precision.U8_NON_LINEAR,
        )

    def release(self, token: str) -> None:
        """Release only the offscreen image owned by this batch."""
        snapshot = self._snapshots.get(token)
        if snapshot is not None:
            snapshot.retired = True
            errors: list[Exception] = []
            for projection in tuple(snapshot.projections):
                try:
                    _release_projection(snapshot, projection)
                except Exception as error:
                    errors.append(error)
            if errors:
                raise ExceptionGroup("GIMP projection cleanup failed.", errors)
            _delete_image(snapshot.image)
            del self._snapshots[token]

    def close(self) -> None:
        """Retire all batches even when an individual native cleanup fails."""
        self._closed = True
        errors: list[Exception] = []
        for token in tuple(self._snapshots):
            try:
                self.release(token)
            except Exception as error:
                errors.append(error)
        for image in tuple(self._unadmitted):
            try:
                _delete_image(image)
                self._unadmitted.remove(image)
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("GIMP snapshot shutdown cleanup failed.", errors)

    def _get(self, token: str) -> _Snapshot:
        """Reject released or externally invalidated snapshots before touching native content."""
        snapshot = self._snapshots.get(token)
        if (
            self._closed
            or snapshot is None
            or snapshot.retired
            or not snapshot.image.is_valid()
        ):
            raise StaleNativeImage("The GIMP snapshot is no longer available.")
        return snapshot


def _delete_image(image: NativeImage) -> None:
    """Delete an owned native image while accepting an already invalidated proxy."""
    if image.is_valid() and not image.delete():
        raise RuntimeError(f"GIMP could not release capture image {image.get_id()}.")


def _release_projection(snapshot: _Snapshot, layer: NativeLayer) -> None:
    """Keep failed native releases owned so shutdown or explicit release can retry them."""
    if layer.is_valid() and not layer.delete():
        raise RuntimeError("GIMP could not release the temporary projection layer.")
    snapshot.projections.remove(layer)
