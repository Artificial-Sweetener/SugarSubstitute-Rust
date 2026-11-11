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
"""Normalize ordered provider cache rows and retain independent image ownership."""

from __future__ import annotations

from dataclasses import dataclass, replace
from collections.abc import Sequence
import sqlite3

from ..domain.provider_metadata import (
    ProviderMetadataPresentation,
    ProviderMetadataRecord,
    provider_image_origin,
    project_provider_metadata,
)
from ..domain.provider_links import ModelProviderLink, unique_provider_links
from ..domain.thumbnail_sources import ThumbnailSourceOrigin, thumbnail_source_origin
from .metadata_codec import decode_metadata, encode_metadata
from .catalog_row_projection import catalog_text, catalog_integer
from .provider_link_codec import encode_provider_link, decode_provider_link


@dataclass(frozen=True, slots=True)
class ProviderMetadataRow:
    """Carry validated SQL values for one provider's cache record."""

    provider: str
    presentation_order: int
    link_order: int | None
    payload: str | None
    link_payload: str | None
    thumbnail_origin: ThumbnailSourceOrigin | None


def provider_metadata_rows(
    records: Sequence[ProviderMetadataRecord],
) -> tuple[ProviderMetadataRow, ...]:
    """Preserve first-match presentation and independent ordered links without false metadata."""
    selected: dict[str, ProviderMetadataRecord] = {}
    for record in records:
        metadata = decode_metadata(encode_metadata(record.metadata))
        selected.setdefault(
            metadata.provider.casefold() or "legacy",
            ProviderMetadataRecord(metadata, record.thumbnail_origin),
        )
    links = unique_provider_links(
        link for record in selected.values() for link in record.metadata.links()
    )
    ordered_links = {
        link.provider_id.casefold(): (index, link) for index, link in enumerate(links)
    }
    positions = {provider: index for index, provider in enumerate(selected)}
    rows: list[ProviderMetadataRow] = []
    for provider in dict.fromkeys((*selected, *ordered_links)):
        selected_record = selected.get(provider)
        link = ordered_links.get(provider)
        origin = provider_image_origin(selected_record) if selected_record else None
        rows.append(
            ProviderMetadataRow(
                provider,
                positions.get(provider, len(selected)),
                link[0] if link else None,
                (
                    encode_metadata(
                        replace(selected_record.metadata, provider_links=())
                    )
                    if selected_record
                    else None
                ),
                encode_provider_link(link[1]) if link else None,
                origin,
            )
        )
    return tuple(rows)


def stored_provider_metadata_row(row: sqlite3.Row) -> ProviderMetadataRow:
    """Reject corrupt cache state before it crosses into the catalog projection."""
    link_order: object = row["link_order"]
    payload: object = row["payload"]
    link_payload: object = row["link_payload"]
    origin: object = row["thumbnail_origin"]
    if (
        (link_order is not None and (not isinstance(link_order, int) or link_order < 0))
        or (payload is not None and not isinstance(payload, str))
        or (link_payload is not None and not isinstance(link_payload, str))
        or (payload is None and link_payload is None)
        or ((link_order is None) != (link_payload is None))
    ):
        raise ValueError("Invalid stored provider metadata row.")
    provider = catalog_text(row, "provider")
    position = catalog_integer(row, "presentation_order")
    if not provider or provider != provider.casefold() or position < 0:
        raise ValueError("Invalid stored provider metadata identity.")
    if payload is not None:
        metadata = decode_metadata(payload)
        if (
            metadata.provider_links
            or (metadata.provider.casefold() or "legacy") != provider
        ):
            raise ValueError("Stored provider metadata has conflicting ownership.")
        if bool(metadata.thumbnails) != (origin is not None):
            raise ValueError(
                "Stored provider image has missing or conflicting ownership."
            )
    elif origin is not None:
        raise ValueError("Stored association cannot own an image without metadata.")
    if (
        link_payload is not None
        and decode_provider_link(link_payload).provider_id.casefold() != provider
    ):
        raise ValueError("Stored provider association has conflicting ownership.")
    return ProviderMetadataRow(
        provider,
        position,
        link_order,
        payload,
        link_payload,
        thumbnail_source_origin(origin) if origin is not None else None,
    )


def provider_metadata_presentation(
    rows: Sequence[ProviderMetadataRow],
) -> ProviderMetadataPresentation:
    """Apply reference provider ordering and CivitAI image fallback independently from labels."""
    links: tuple[ModelProviderLink, ...] = tuple(
        decode_provider_link(item.link_payload)
        for item in sorted(
            rows,
            key=lambda item: (
                item.link_order if item.link_order is not None else len(rows)
            ),
        )
        if item.link_payload is not None
    )
    presentations = sorted(
        (item for item in rows if item.payload is not None),
        key=lambda item: item.presentation_order,
    )
    return project_provider_metadata(
        tuple(
            ProviderMetadataRecord(decode_metadata(item.payload), item.thumbnail_origin)
            for item in presentations
            if item.payload is not None
        ),
        links,
    )
