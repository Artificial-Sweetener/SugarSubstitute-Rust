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
"""Migrate combined provider payloads into independently owned cache rows."""

from __future__ import annotations

import sqlite3

from ..domain.provider_metadata import ProviderMetadataRecord
from .catalog_row_projection import catalog_text
from .metadata_codec import decode_metadata, normalized_hash
from .provider_metadata_codec import provider_metadata_rows
from .provider_metadata_store import write_provider_metadata_rows


def migrate_provider_metadata(connection: sqlite3.Connection) -> None:
    """Preserve legacy presentation and migrate every row in the schema transaction."""
    connection.execute("""CREATE TABLE IF NOT EXISTS model_provider_metadata (
        sha256 TEXT NOT NULL, provider TEXT NOT NULL, presentation_order INTEGER NOT NULL,
        link_order INTEGER, payload TEXT, link_payload TEXT, thumbnail_origin TEXT,
        PRIMARY KEY(sha256,provider)
    )""")
    for row in connection.execute(
        "SELECT sha256,payload FROM model_metadata WHERE owner='provider'"
    ).fetchall():
        sha256 = normalized_hash(catalog_text(row, "sha256"))
        records = provider_metadata_rows(
            (ProviderMetadataRecord(decode_metadata(catalog_text(row, "payload"))),)
        )
        connection.execute(
            "DELETE FROM model_provider_metadata WHERE sha256=?", (sha256,)
        )
        write_provider_metadata_rows(connection, sha256, records)
    connection.execute("DELETE FROM model_metadata WHERE owner='provider'")
