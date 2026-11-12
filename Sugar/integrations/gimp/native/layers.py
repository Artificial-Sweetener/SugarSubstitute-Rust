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
"""Read the native hierarchy without reproducing shared layer-range policy."""

from __future__ import annotations

from collections.abc import Sequence

from .image_api import NativeImage, NativeLayer
from .model import NativeLayerInfo


def layer_catalog(image: NativeImage) -> tuple[NativeLayerInfo, ...]:
    """Return a top-to-bottom preorder catalog with explicit parent identities."""
    result: list[NativeLayerInfo] = []
    seen: set[int] = set()

    pending: list[tuple[NativeLayer, int | None, int]] = [
        (layer, None, 0) for layer in reversed(image.get_layers())
    ]
    while pending:
        layer, parent, depth = pending.pop()
        if len(result) >= 4096 or depth > 128:
            raise RuntimeError(
                "The GIMP layer catalog exceeds its supported size or depth."
            )
        tattoo = layer.get_tattoo()
        if tattoo < 1 or tattoo in seen:
            raise RuntimeError(
                "The GIMP layer hierarchy contains duplicate identities."
            )
        seen.add(tattoo)
        result.append(
            NativeLayerInfo(
                tattoo,
                parent,
                layer.get_name(),
                depth,
                layer.get_visible(),
                layer.get_mask() is not None,
            )
        )
        if layer.is_group():
            children: Sequence[NativeLayer] = layer.get_children() or ()
            pending.extend((child, tattoo, depth + 1) for child in reversed(children))
    return tuple(result)


def primary_layer(image: NativeImage) -> int | None:
    """Read the first selected native layer inside this exact image."""
    selected = image.get_selected_layers()
    return selected[0].get_tattoo() if selected else None
