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

from collections.abc import Callable, Sequence
from dataclasses import dataclass, replace

from .models import ModelMetadata, PreviewCandidate, ThumbnailReference
from .provider_links import ModelProviderLink, unique_provider_links
from .provider_models import ProviderModelVersion
from .thumbnail_safety import EXPLICIT_TIER
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
    previews: dict[str, PreviewCandidate] = {}
    for record in records:
        for preview in record.metadata.previews:
            previews.setdefault(preview.url, preview)
    metadata = replace(
        primary.metadata,
        thumbnails=image_record.metadata.thumbnails,
        thumbnail_tier=image_record.metadata.thumbnail_tier,
        previews=tuple(previews.values()),
        provider_links=ordered_links,
    )
    return ProviderMetadataPresentation(
        metadata, ordered_links, provider_image_origin(image_record)
    )


def merge_provider_records(
    stored: Sequence[ProviderMetadataRecord],
    versions: Sequence[ProviderModelVersion],
    *,
    replace_labels: bool,
) -> tuple[ProviderMetadataRecord, ...]:
    """Add what a lookup found to what a model already has, removing nothing.

    A provider that no longer answers for the file keeps its stored record, and a stored image
    is never replaced. ``replace_labels`` lets a refresh a person asked for update the names
    and words of a provider that answered again; an automatic lookup leaves them as learned.
    """
    merged = {_record_key(record): record for record in stored}
    for version in versions:
        key = version.provider.casefold() or "legacy"
        old = merged.get(key)
        if old is None:
            merged[key] = ProviderMetadataRecord(version.metadata(()))
            continue
        previews = merge_previews(old.metadata.previews, version.previews)
        merged[key] = ProviderMetadataRecord(
            (
                version.metadata(
                    old.metadata.thumbnails,
                    thumbnail_tier=old.metadata.thumbnail_tier,
                    previews=previews,
                )
                if replace_labels
                else replace(old.metadata, previews=previews)
            ),
            old.thumbnail_origin,
        )
    return tuple(merged.values())


def merge_previews(
    stored: Sequence[PreviewCandidate], offered: Sequence[PreviewCandidate]
) -> tuple[PreviewCandidate, ...]:
    """Take the provider's current offer while keeping every image already fetched."""
    fetched = {item.url: item for item in stored if item.thumbnails}
    merged = [fetched.pop(item.url, item) for item in offered]
    merged.extend(fetched.values())
    return tuple(merged)


def with_fetched_preview(
    records: Sequence[ProviderMetadataRecord],
    url: str,
    thumbnails: tuple[ThumbnailReference, ...],
) -> tuple[ProviderMetadataRecord, ...]:
    """Attach prepared files to the image they came from; a model with no image adopts them."""
    updated: list[ProviderMetadataRecord] = []
    for record in records:
        metadata = record.metadata
        candidate = next((item for item in metadata.previews if item.url == url), None)
        if candidate is None:
            updated.append(record)
            continue
        metadata = replace(
            metadata,
            previews=tuple(
                replace(item, thumbnails=thumbnails) if item.url == url else item
                for item in metadata.previews
            ),
        )
        origin = record.thumbnail_origin
        if not metadata.thumbnails:
            metadata = replace(
                metadata, thumbnails=thumbnails, thumbnail_tier=candidate.tier
            )
            origin = None
        updated.append(ProviderMetadataRecord(metadata, origin))
    return tuple(updated)


def with_blurred_variants(
    records: Sequence[ProviderMetadataRecord],
    sharp: tuple[ThumbnailReference, ...],
    variants: tuple[ThumbnailReference, ...],
) -> tuple[ProviderMetadataRecord, ...]:
    """Store blurred files beside the sharp image they were made from, wherever it is kept."""

    def extended(
        thumbnails: tuple[ThumbnailReference, ...],
    ) -> tuple[ThumbnailReference, ...]:
        """Replace any earlier blurred files of the same sharp image."""
        kept = tuple(item for item in thumbnails if "blurred" not in item.role)
        return (*kept, *variants) if kept == sharp else thumbnails

    return tuple(
        ProviderMetadataRecord(
            replace(
                record.metadata,
                thumbnails=extended(record.metadata.thumbnails),
                previews=tuple(
                    replace(item, thumbnails=extended(item.thumbnails))
                    for item in record.metadata.previews
                ),
            ),
            record.thumbnail_origin,
        )
        for record in records
    )


def with_offered_previews(
    records: Sequence[ProviderMetadataRecord],
    provider: str,
    offered: Sequence[PreviewCandidate],
) -> tuple[ProviderMetadataRecord, ...]:
    """Record what a provider offers for a card learned before offers were stored.

    Such a card's image was the first one its setting allowed. An image whose tier is unknown
    takes the tier of the first offered image, which is the one an unrestricted setting chose.
    """
    updated: list[ProviderMetadataRecord] = []
    for record in records:
        metadata = record.metadata
        if _record_key(record) != provider or metadata.previews:
            updated.append(record)
            continue
        tier = metadata.thumbnail_tier
        if metadata.thumbnails and offered and tier in (None, EXPLICIT_TIER):
            tier = offered[0].tier
        updated.append(
            ProviderMetadataRecord(
                replace(metadata, thumbnail_tier=tier, previews=tuple(offered)),
                record.thumbnail_origin,
            )
        )
    return tuple(updated)


def without_lost_images(
    records: Sequence[ProviderMetadataRecord],
    kept: Callable[[tuple[ThumbnailReference, ...]], bool],
) -> tuple[ProviderMetadataRecord, ...]:
    """Forget stored images whose files no longer exist, leaving every label and offer.

    ``kept`` says whether all of an image's files are still on disk. A forgotten image is
    owed again from the stored offer; nothing is asked of a provider.
    """
    updated: list[ProviderMetadataRecord] = []
    for record in records:
        metadata = record.metadata
        lost = bool(metadata.thumbnails) and not kept(metadata.thumbnails)
        updated.append(
            ProviderMetadataRecord(
                replace(
                    metadata,
                    thumbnails=() if lost else metadata.thumbnails,
                    thumbnail_tier=None if lost else metadata.thumbnail_tier,
                    previews=tuple(
                        (
                            replace(item, thumbnails=())
                            if item.thumbnails and not kept(item.thumbnails)
                            else item
                        )
                        for item in metadata.previews
                    ),
                ),
                None if lost else record.thumbnail_origin,
            )
        )
    return tuple(updated)


def _record_key(record: ProviderMetadataRecord) -> str:
    """Name a stored record by its provider, as its storage row does."""
    return record.metadata.provider.casefold() or "legacy"
