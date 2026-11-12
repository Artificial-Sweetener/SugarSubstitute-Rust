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
"""Persist user maturity choices by verified bytes without replacing provider evidence."""

from __future__ import annotations

from dataclasses import dataclass

from .catalog_database import ModelCatalogDatabase
from .metadata_codec import normalized_hash


@dataclass(frozen=True, slots=True)
class SqliteModelContentOverrides:
    """Own independent override records and their catalog invalidation transaction."""

    database: ModelCatalogDatabase

    def read(self, sha256: str) -> bool | None:
        """Read an explicit user choice without altering provider classification."""
        with self.database.connect() as connection:
            row = connection.execute(
                "SELECT nsfw FROM model_content_overrides WHERE sha256=?",
                (normalized_hash(sha256),),
            ).fetchone()
            if row is None:
                return None
            value = row["nsfw"]
            if type(value) is not int or value not in (0, 1):
                raise ValueError("Invalid stored model content classification.")
            return value == 1

    def save(self, sha256: str, nsfw: bool | None) -> None:
        """Clear to provider classification or retain an explicit boolean by content identity."""
        digest = normalized_hash(sha256)
        with self.database.connect() as connection:
            if nsfw is None:
                connection.execute(
                    "DELETE FROM model_content_overrides WHERE sha256=?", (digest,)
                )
            else:
                connection.execute(
                    "INSERT INTO model_content_overrides VALUES (?,?) ON CONFLICT(sha256) DO UPDATE SET nsfw=excluded.nsfw",
                    (digest, int(nsfw)),
                )
            self.database.advance_revision(connection)
