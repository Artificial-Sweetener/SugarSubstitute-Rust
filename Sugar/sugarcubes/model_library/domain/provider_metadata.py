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
"""Keep provider evidence and image provenance independent from chosen presentation."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, replace

from .models import ModelMetadata
from .provider_links import ModelProviderLink, unique_provider_links
from .thumbnail_sources import ThumbnailSourceOrigin, thumbnail_source_origin


@dataclass(frozen=True, slots=True)
class ProviderMetadataRecord:
    """Carry one exact provider match with the owner of its selected image bytes."""

    metadata: ModelMetadata
    thumbnail_origin: ThumbnailSourceOrigin | None = None


@dataclass(frozen=True, slots=True)
class ProviderMetadataPresentation:
    """Project ordered provider evidence without inventing labels for an association-only row."""

    metadata: ModelMetadata | None = None
    links: tuple[ModelProviderLink, ...] = ()
    thumbnail_origin: ThumbnailSourceOrigin | None = None


def provider_image_origin(
    record: ProviderMetadataRecord,
) -> ThumbnailSourceOrigin | None:
    """Retain explicit image ownership while classifying historical provider images conservatively."""
    if not record.metadata.thumbnails:
        return None
    provider = record.metadata.provider.casefold()
    return thumbnail_source_origin(
        record.thumbnail_origin
        or (provider if provider in ("civitai", "openmodeldb") else "legacy")
    )


def project_provider_metadata(
    records: Sequence[ProviderMetadataRecord],
    links: tuple[ModelProviderLink, ...] | None = None,
) -> ProviderMetadataPresentation:
    """Choose first-provider labels and its image, falling back to independent CivitAI evidence."""
    ordered_links = (
        links
        if links is not None
        else unique_provider_links(
            link for record in records for link in record.metadata.links()
        )
    )
    if not records:
        return ProviderMetadataPresentation(links=ordered_links)
    primary = records[0]
    image_record = primary
    if not primary.metadata.thumbnails:
        image_record = next(
            (
                record
                for record in records
                if record.metadata.provider.casefold() == "civitai"
                and record.metadata.thumbnails
            ),
            primary,
        )
    metadata = replace(
        primary.metadata,
        thumbnails=image_record.metadata.thumbnails,
        provider_links=ordered_links,
    )
    return ProviderMetadataPresentation(
        metadata, ordered_links, provider_image_origin(image_record)
    )
