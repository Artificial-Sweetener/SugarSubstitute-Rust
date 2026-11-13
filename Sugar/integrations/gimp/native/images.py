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
"""Resolve explicit native images and qualify their canvas geometry."""

from __future__ import annotations

from hashlib import sha256
import json
from typing import TypeVar

from .image_api import ImageLookup, NativeImage
from .model import NativeDocument, StaleNativeImage

ImageType = TypeVar("ImageType", bound=NativeImage)


def describe_image(image: NativeImage) -> NativeDocument:
    """Read geometry twice so a native resize cannot produce mixed metadata."""
    if not image.is_valid():
        raise StaleNativeImage("The GIMP image has closed.")
    width, height, revision = _geometry(image)
    name = image.get_name()
    if not image.is_valid() or (width, height, revision) != _geometry(image):
        raise StaleNativeImage("The GIMP canvas changed while its metadata was read.")
    return NativeDocument(image.get_id(), revision, width, height, name)


def resolve_image(
    images: ImageLookup[ImageType], image_id: int, revision: str | None = None
) -> ImageType:
    """Refuse stale proxies and never substitute a later active image."""
    image = images.get_by_id(image_id)
    if image is None or not image.is_valid() or image.get_id() != image_id:
        raise StaleNativeImage("The GIMP image has closed.")
    if revision is not None and describe_image(image).revision != revision:
        raise StaleNativeImage("The GIMP canvas geometry changed.")
    return image


def resolve_document(
    images: ImageLookup[ImageType], expected: NativeDocument
) -> ImageType:
    """Require every captured coordinate qualifier before preparing a native mutation."""
    image = resolve_image(images, expected.image_id)
    current = describe_image(image)
    if (current.revision, current.width, current.height) != (
        expected.revision,
        expected.width,
        expected.height,
    ):
        raise StaleNativeImage("The GIMP destination geometry changed.")
    return image


def _geometry(image: NativeImage) -> tuple[int, int, str]:
    """Give capture and output insertion one native geometry vocabulary."""
    width, height = image.get_width(), image.get_height()
    if width < 1 or height < 1:
        raise StaleNativeImage("The GIMP canvas has invalid dimensions.")
    profile = sha256(image.get_effective_color_profile().get_icc_profile()).hexdigest()
    revision = json.dumps(
        [
            width,
            height,
            int(image.get_base_type()),
            int(image.get_precision()),
            profile,
        ],
        separators=(",", ":"),
    )
    return width, height, revision
