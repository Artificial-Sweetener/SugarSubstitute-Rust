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
"""Validate explicit decision ownership and migrate known historical provider scopes."""

from __future__ import annotations

import json

from ..domain.models import ModelMetadata
from ..domain.thumbnail_sources import ThumbnailSourceOrigin, thumbnail_source_origin


def decode_enrichment_providers(payload: object) -> tuple[str, ...]:
    """Keep malformed persisted ownership from addressing unrelated provider caches."""
    if not isinstance(payload, str):
        raise ValueError("Invalid enrichment provider ownership.")
    value: object = json.loads(payload)
    if not isinstance(value, list) or not all(
        isinstance(item, str) and item and len(item) <= 128 for item in value
    ):
        raise ValueError("Invalid enrichment provider ownership.")
    return tuple(
        dict.fromkeys(item.casefold() for item in value if isinstance(item, str))
    )


def legacy_enrichment_providers(scope: str) -> tuple[str, ...]:
    """Recognize only the three historical production scopes; retain unknown evidence untouched."""
    return {
        "civitai": ("civitai",),
        "openmodeldb": ("openmodeldb",),
        "openmodeldb,civitai": ("openmodeldb", "civitai"),
    }.get(scope, ())


def enrichment_thumbnail_origin(
    value: object, metadata: ModelMetadata | None, local_preview: bool
) -> ThumbnailSourceOrigin | None:
    """Migrate historical image ownership without attributing protected local images to label providers."""
    if value is not None:
        origin = thumbnail_source_origin(value)
        if metadata is None or not metadata.thumbnails:
            raise ValueError("Enrichment image origin has no image.")
        return origin
    if metadata is None or not metadata.thumbnails:
        return None
    if local_preview or not metadata.provider:
        return "local"
    provider = metadata.provider.casefold()
    return thumbnail_source_origin(
        provider if provider in ("civitai", "openmodeldb") else "legacy"
    )
