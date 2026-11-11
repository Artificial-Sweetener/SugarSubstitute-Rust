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
"""Validate catalog rows and project user presentation with independent provider evidence."""

from __future__ import annotations

from dataclasses import replace
import sqlite3

from ..domain.models import ModelCatalogEntry, ModelEvidence, ModelMetadata
from ..domain.architecture import ModelArchitecture
from ..domain.provider_links import unique_provider_links
from ..domain.provider_metadata import ProviderMetadataPresentation
from ..domain.thumbnail_sources import ThumbnailSourceOrigin
from .metadata_codec import decode_metadata, normalized_hash


def catalog_text(row: sqlite3.Row, name: str) -> str:
    """Narrow dynamic SQLite values before creating domain records."""
    value: object = row[name]
    if not isinstance(value, str):
        raise ValueError(f"Invalid model catalog text: {name}.")
    return value


def catalog_integer(row: sqlite3.Row, name: str) -> int:
    """Reject corrupt numeric evidence rather than silently coercing it."""
    value: object = row[name]
    if not isinstance(value, int):
        raise ValueError(f"Invalid model catalog integer: {name}.")
    return value


def catalog_entry(
    row: sqlite3.Row, provider: ProviderMetadataPresentation
) -> ModelCatalogEntry:
    """Project a validated stored location with optional hash-based enrichment."""
    evidence = ModelEvidence(
        kind=catalog_text(row, "kind"),
        value=catalog_text(row, "value"),
        root_id=catalog_text(row, "root_id"),
        relative_path=catalog_text(row, "relative_path"),
        size_bytes=catalog_integer(row, "size_bytes"),
        modified_ns=catalog_integer(row, "modified_ns"),
    )
    fingerprint: object = row["sha256"]
    if fingerprint is not None and not isinstance(fingerprint, str):
        raise ValueError("Invalid stored model fingerprint.")
    user = stored_metadata(row, "user_metadata")
    local = stored_metadata(row, "local_metadata")
    decoded = user or provider.metadata or local
    if decoded and provider.metadata:
        decoded = replace(
            decoded,
            provider_nsfw=provider.metadata.provider_nsfw,
            provider_nsfw_level=provider.metadata.provider_nsfw_level,
            model_type=provider.metadata.model_type,
        )
    origin: ThumbnailSourceOrigin | None = (
        "user"
        if user is not None
        else (
            provider.thumbnail_origin
            if provider.metadata is not None
            else "local" if local is not None else None
        )
    )
    if provider.links:
        decoded = replace(
            decoded or ModelMetadata(title=evidence.file_title),
            provider_links=(
                unique_provider_links((*decoded.provider_links, *provider.links))
                if decoded is not None
                else provider.links
            ),
        )
    choice: object = row["chosen_thumbnail"]
    if choice is not None:
        if not isinstance(choice, str):
            raise ValueError("Invalid chosen model thumbnail.")
        decoded = replace(
            decoded or ModelMetadata(title=evidence.file_title),
            thumbnails=decode_metadata(choice).thumbnails,
        )
        origin = "user"
    return ModelCatalogEntry(
        evidence=evidence,
        sha256=normalized_hash(fingerprint) if fingerprint is not None else None,
        metadata=decoded,
        thumbnail_origin=origin if decoded is not None and decoded.thumbnails else None,
        architecture=(
            ModelArchitecture(
                catalog_text(row, "architecture_base"),
                catalog_text(row, "architecture_class"),
            )
            if row["architecture_base"] is not None
            else None
        ),
        nsfw_override=_nsfw_override(row["nsfw_override"]),
    )


def stored_metadata(row: sqlite3.Row, name: str) -> ModelMetadata | None:
    """Narrow optional persisted presentation at the catalog read boundary."""
    value: object = row[name]
    if value is not None and not isinstance(value, str):
        raise ValueError("Invalid stored model metadata.")
    return decode_metadata(value) if value is not None else None


def _nsfw_override(value: object) -> bool | None:
    """Reject corrupt stored classifications instead of coercing arbitrary integers."""
    if value is None:
        return None
    if not isinstance(value, int) or value not in (0, 1):
        raise ValueError("Invalid stored model content classification.")
    return bool(value)
