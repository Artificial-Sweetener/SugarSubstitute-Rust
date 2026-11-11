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
"""Persist the shared organizing pattern inside the model-library namespace."""

from __future__ import annotations

from dataclasses import dataclass

from ..application.organized_downloads import validate_download_pattern
from .catalog_database import ModelCatalogDatabase
from .catalog_row_projection import catalog_text


@dataclass(frozen=True, slots=True)
class SqliteDownloadPreferences:
    """Own persistence while reusing path validation at the storage boundary."""

    database: ModelCatalogDatabase

    def read(self) -> str:
        """Return the normal filename default for existing catalogs."""
        with self.database.connect() as connection:
            row = connection.execute(
                "SELECT pattern FROM model_download_preferences WHERE id=1"
            ).fetchone()
            return (
                validate_download_pattern(catalog_text(row, "pattern"))
                if row
                else "{file_name}"
            )

    def save(self, pattern: str) -> None:
        """Reject invalid patterns before committing a settings change."""
        normalized = validate_download_pattern(pattern)
        with self.database.connect() as connection:
            connection.execute(
                "INSERT INTO model_download_preferences VALUES (1,?) ON CONFLICT(id) DO UPDATE SET pattern=excluded.pattern",
                (normalized,),
            )
