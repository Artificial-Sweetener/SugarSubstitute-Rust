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
"""Publish prepared catalog changes when cached release families change."""

from __future__ import annotations

from pathlib import Path

from ..application.release_history import ReleaseHistorySnapshot
from .catalog_database import ModelCatalogDatabase
from .release_history_store import SqliteReleaseHistory


class CatalogReleaseHistory(SqliteReleaseHistory):
    """Keep picker availability synchronized with the shared durable release cache."""

    def __init__(self, path: Path, catalog: ModelCatalogDatabase) -> None:
        """Bind history persistence to the catalog's authoritative change token."""
        super().__init__(path)
        self._catalog = catalog

    def save(self, key: str, snapshot: ReleaseHistorySnapshot) -> None:
        """Invalidate prepared counts only when successfully committed release evidence changes."""
        previous = self.read(key)
        super().save(key, snapshot)
        if previous is None or previous.releases != snapshot.releases:
            self._publish()

    def clear(self) -> None:
        """Invalidate prepared counts after provider history is removed."""
        super().clear()
        self._publish()

    def _publish(self) -> None:
        """Advance the persisted revision consumed by every catalog watcher."""
        with self._catalog.connect() as connection, connection:
            self._catalog.advance_revision(connection)
