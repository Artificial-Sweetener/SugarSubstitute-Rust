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
"""Decide which stored image a model shows under the current thumbnail safety setting.

A model keeps the image it was first shown with. The setting never changes what is stored: it
chooses among the stored images, so loosening or tightening it needs no provider lookup.
"""

from __future__ import annotations

from dataclasses import dataclass

from .models import ModelCatalogEntry, PreviewCandidate, ThumbnailReference
from .thumbnail_safety import (
    EXPLICIT_TIER,
    CivitaiThumbnailSafety,
    blurs_tier,
    downloads_tier,
)

_PROVIDER_ORIGINS = ("civitai", "openmodeldb", "legacy")
_BLURRED = {"standard-blurred": "standard", "banner-blurred": "banner"}


@dataclass(frozen=True, slots=True)
class ThumbnailPresentation:
    """What a model shows, and what is still owed before it can show more.

    ``thumbnails`` carry the roles a consumer draws ("standard", "banner"); when ``blurred``
    they are the blurred files and ``reveal`` holds the sharp ones. ``wanted`` is the provider
    image to fetch for a model that shows nothing yet. ``unblurred`` are sharp files whose
    blurred variants have not been made. ``filtered`` says an image exists that the setting
    excludes and nothing else can take its place.
    """

    thumbnails: tuple[ThumbnailReference, ...] = ()
    reveal: tuple[ThumbnailReference, ...] = ()
    blurred: bool = False
    wanted: PreviewCandidate | None = None
    unblurred: tuple[ThumbnailReference, ...] = ()
    filtered: bool = False


def sharp_thumbnails(
    thumbnails: tuple[ThumbnailReference, ...],
) -> tuple[ThumbnailReference, ...]:
    """Select the files drawn as they were prepared."""
    return tuple(item for item in thumbnails if item.role not in _BLURRED)


def blurred_thumbnails(
    thumbnails: tuple[ThumbnailReference, ...],
) -> tuple[ThumbnailReference, ...]:
    """Select the blurred files under the roles a consumer draws."""
    return tuple(
        ThumbnailReference(
            item.asset_id,
            _BLURRED[item.role],
            item.width,
            item.height,
            item.content_type,
        )
        for item in thumbnails
        if item.role in _BLURRED
    )


def present_thumbnail(
    entry: ModelCatalogEntry, policy: CivitaiThumbnailSafety
) -> ThumbnailPresentation:
    """Choose the first stored image the setting allows, in the order a model acquired them."""
    metadata = entry.metadata
    if metadata is None:
        return ThumbnailPresentation()
    provider_owned = entry.thumbnail_origin in _PROVIDER_ORIGINS
    if metadata.thumbnails and not provider_owned:
        return ThumbnailPresentation(sharp_thumbnails(metadata.thumbnails))
    stored: list[tuple[tuple[ThumbnailReference, ...], int]] = []
    if metadata.thumbnails:
        stored.append(
            (
                metadata.thumbnails,
                (
                    metadata.thumbnail_tier
                    if metadata.thumbnail_tier is not None
                    else EXPLICIT_TIER
                ),
            )
        )
    first = sharp_thumbnails(metadata.thumbnails)
    stored.extend(
        (item.thumbnails, item.tier)
        for item in metadata.previews
        if item.thumbnails and sharp_thumbnails(item.thumbnails) != first
    )
    for thumbnails, tier in stored:
        if not downloads_tier(policy, tier):
            continue
        sharp = sharp_thumbnails(thumbnails)
        if not blurs_tier(policy, tier):
            return ThumbnailPresentation(sharp)
        blurred = blurred_thumbnails(thumbnails)
        if blurred:
            return ThumbnailPresentation(blurred, reveal=sharp, blurred=True)
        return ThumbnailPresentation(unblurred=sharp, blurred=True)
    if entry.local_thumbnails:
        return ThumbnailPresentation(sharp_thumbnails(entry.local_thumbnails))
    wanted = next(
        (
            item
            for item in metadata.previews
            if not item.thumbnails and downloads_tier(policy, item.tier)
        ),
        None,
    )
    return ThumbnailPresentation(
        wanted=wanted,
        filtered=wanted is None and bool(stored or metadata.previews),
    )
