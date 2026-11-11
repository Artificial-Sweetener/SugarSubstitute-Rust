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
"""Persist independent provider records and maintain only their owned cache state."""

from __future__ import annotations

from dataclasses import replace
import sqlite3
from typing import TYPE_CHECKING

from ..domain.models import ModelMetadata
from ..domain.provider_metadata import (
    ProviderMetadataPresentation,
    ProviderMetadataRecord,
)
from ..domain.thumbnail_sources import ThumbnailOwnership, ThumbnailSourceOrigin
from .catalog_row_projection import catalog_text
from .metadata_codec import decode_metadata, encode_metadata, normalized_hash
from .provider_metadata_codec import (
    ProviderMetadataRow,
    provider_metadata_rows,
    stored_provider_metadata_row,
    provider_metadata_presentation,
)

if TYPE_CHECKING:
    from .catalog_database import ModelCatalogDatabase


def write_provider_metadata_rows(
    connection: sqlite3.Connection, sha256: str, rows: tuple[ProviderMetadataRow, ...]
) -> None:
    """Write validated provider rows inside the authoritative catalog transaction."""
    connection.executemany(
        "INSERT INTO model_provider_metadata VALUES (?,?,?,?,?,?,?)",
        (
            (
                sha256,
                item.provider,
                item.presentation_order,
                item.link_order,
                item.payload,
                item.link_payload,
                item.thumbnail_origin,
            )
            for item in rows
        ),
    )


class SqliteModelProviderMetadata:
    """Share catalog transactions while keeping provider maintenance out of user-owned records."""

    def __init__(self, database: ModelCatalogDatabase) -> None:
        """Use the catalog's authoritative namespace and committed revision owner."""
        self._database = database

    def replace(
        self,
        sha256: str,
        records: tuple[ProviderMetadataRecord, ...],
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Publish an exact-hash match batch and one invalidation token atomically."""
        fingerprint = normalized_hash(sha256)
        rows = provider_metadata_rows(records)
        with self._database.connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            old = self._rows(connection, fingerprint)
            if old == rows:
                if prepared_assets_changed:
                    self._database.advance_revision(connection)
                return
            connection.execute(
                "DELETE FROM model_provider_metadata WHERE sha256=?", (fingerprint,)
            )
            write_provider_metadata_rows(connection, fingerprint, rows)
            self._database.advance_revision(connection)

    def metadata(self, sha256: str, provider: str) -> ModelMetadata | None:
        """Return only a provider's actual labels; association-only evidence supplies no labels."""
        with self._database.connect() as connection:
            rows = self._rows(connection, normalized_hash(sha256))
            return next(
                (
                    decode_metadata(row.payload)
                    for row in rows
                    if row.provider == provider.casefold() and row.payload is not None
                ),
                None,
            )

    def count(self, provider: str) -> int:
        """Count exact provider associations even when legacy evidence retained only a link."""
        with self._database.connect() as connection:
            return len(
                tuple(
                    connection.execute(
                        "SELECT sha256 FROM model_provider_metadata WHERE provider=?",
                        (provider.casefold(),),
                    )
                )
            )

    def clear_metadata(self, provider: str) -> None:
        """Remove one provider's evidence and images borrowed from it while retaining other matches."""
        with self._database.connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            cursor = connection.execute(
                "DELETE FROM model_provider_metadata WHERE provider=?",
                (provider.casefold(),),
            )
            changed = self._clear_thumbnails(connection, provider.casefold())
            if cursor.rowcount or changed:
                self._database.advance_revision(connection)

    def clear_thumbnails(self, origin: ThumbnailSourceOrigin) -> None:
        """Strip only images owned by a source without removing provider decisions or links."""
        with self._database.connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            if self._clear_thumbnails(connection, origin):
                self._database.advance_revision(connection)

    def delete_for_hash(self, sha256: str) -> None:
        """Drop disproved provider matches independently of local and user presentation."""
        with self._database.connect() as connection:
            cursor = connection.execute(
                "DELETE FROM model_provider_metadata WHERE sha256=?",
                (normalized_hash(sha256),),
            )
            if cursor.rowcount:
                self._database.advance_revision(connection)

    def presentation(
        self, connection: sqlite3.Connection, sha256: str | None
    ) -> ProviderMetadataPresentation:
        """Join provider evidence inside the caller's catalog read transaction."""
        return (
            provider_metadata_presentation(
                self._rows(connection, normalized_hash(sha256))
            )
            if sha256 is not None
            else ProviderMetadataPresentation()
        )

    def presentations(
        self, connection: sqlite3.Connection
    ) -> dict[str, ProviderMetadataPresentation]:
        """Read independent records in one query for a transactionally consistent snapshot."""
        rows: dict[str, list[ProviderMetadataRow]] = {}
        for row in connection.execute(
            "SELECT * FROM model_provider_metadata ORDER BY sha256,presentation_order,link_order,provider"
        ):
            rows.setdefault(normalized_hash(catalog_text(row, "sha256")), []).append(
                stored_provider_metadata_row(row)
            )
        return {
            sha256: provider_metadata_presentation(batch)
            for sha256, batch in rows.items()
        }

    def thumbnail_ownership(
        self, connection: sqlite3.Connection
    ) -> tuple[ThumbnailOwnership, ...]:
        """Retain every raw provider image, including matches hidden by presentation precedence."""
        ownership: list[ThumbnailOwnership] = []
        for row in connection.execute("SELECT * FROM model_provider_metadata"):
            record = stored_provider_metadata_row(row)
            if record.payload is not None and record.thumbnail_origin is not None:
                ownership.append(
                    ThumbnailOwnership(
                        record.thumbnail_origin,
                        decode_metadata(record.payload).thumbnails,
                    )
                )
        return tuple(ownership)

    def _rows(
        self, connection: sqlite3.Connection, sha256: str
    ) -> tuple[ProviderMetadataRow, ...]:
        """Normalize cache rows before reads, comparisons or maintenance address them."""
        return tuple(
            stored_provider_metadata_row(row)
            for row in connection.execute(
                "SELECT * FROM model_provider_metadata WHERE sha256=? ORDER BY presentation_order,link_order,provider",
                (sha256,),
            )
        )

    def _clear_thumbnails(self, connection: sqlite3.Connection, origin: str) -> bool:
        """Rewrite borrowed-image metadata in the same transaction as the provider operation."""
        rows = connection.execute(
            "SELECT * FROM model_provider_metadata WHERE thumbnail_origin=?", (origin,)
        ).fetchall()
        for row in rows:
            item = stored_provider_metadata_row(row)
            if item.payload is None:
                raise ValueError("Provider image has no metadata.")
            payload = encode_metadata(
                replace(decode_metadata(item.payload), thumbnails=())
            )
            connection.execute(
                "UPDATE model_provider_metadata SET payload=?,thumbnail_origin=NULL WHERE sha256=? AND provider=?",
                (payload, catalog_text(row, "sha256"), item.provider),
            )
        return bool(rows)
