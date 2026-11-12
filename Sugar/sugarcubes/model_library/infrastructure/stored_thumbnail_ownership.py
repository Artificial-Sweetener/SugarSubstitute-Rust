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
"""Read persisted thumbnail ownership independently from visible catalog precedence."""

from __future__ import annotations

from ..domain.thumbnail_sources import ThumbnailOwnership, ThumbnailSourceOrigin
from .catalog_database import ModelCatalogDatabase
from .catalog_row_projection import catalog_text
from .metadata_codec import decode_metadata
from .provider_metadata_store import SqliteModelProviderMetadata


class StoredThumbnailOwnership:
    """Retain every known source while leaving unclassified legacy images protected."""

    def __init__(self, database: ModelCatalogDatabase) -> None:
        """Use the authoritative schema and transaction owner."""
        self._database = database

    def read(self) -> tuple[ThumbnailOwnership, ...]:
        """Include raw local and chosen images even when a provider hides them in the catalog."""
        ownership: list[ThumbnailOwnership] = []
        with self._database.connect() as connection:
            connection.execute("BEGIN")
            for row in connection.execute("SELECT owner,payload FROM model_metadata"):
                metadata = decode_metadata(catalog_text(row, "payload"))
                owner = catalog_text(row, "owner")
                if owner != "user":
                    raise ValueError("Unknown stored model metadata owner.")
                ownership.append(ThumbnailOwnership("user", metadata.thumbnails))
            ownership.extend(
                SqliteModelProviderMetadata(self._database).thumbnail_ownership(
                    connection
                )
            )
            origins: tuple[tuple[str, ThumbnailSourceOrigin], ...] = (
                ("local_metadata", "local"),
                ("thumbnail_choices", "user"),
            )
            for table, origin in origins:
                for row in connection.execute(f"SELECT payload FROM {table}"):
                    metadata = decode_metadata(catalog_text(row, "payload"))
                    ownership.append(ThumbnailOwnership(origin, metadata.thumbnails))
        return tuple(ownership)
