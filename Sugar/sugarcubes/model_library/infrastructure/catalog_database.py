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
"""Own catalog schema, transaction leases and committed revision state."""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
import sqlite3

from .catalog_row_projection import catalog_integer
from .provider_metadata_migration import migrate_provider_metadata


class ModelCatalogDatabase:
    """Share one versioned catalog namespace among focused persistence adapters."""

    def __init__(self, model_root: Path) -> None:
        """Initialize the established schema without replacing unsupported user data."""
        self.asset_root = (
            model_root.resolve() / ".sugarcubes" / "model-library" / "v1" / "assets"
        )
        self.asset_root.parent.mkdir(parents=True, exist_ok=True)
        self._path = self.asset_root.parent / "catalog.sqlite3"
        with self.connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            version = connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, 1):
                raise ValueError(
                    "Unsupported model catalog schema; original data was preserved."
                )
            schema = """
                CREATE TABLE IF NOT EXISTS catalog_state (
                    id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL
                );
                INSERT OR IGNORE INTO catalog_state VALUES (1,0);
                CREATE TABLE IF NOT EXISTS model_files (
                    id TEXT PRIMARY KEY, kind TEXT NOT NULL, value TEXT NOT NULL,
                    root_id TEXT NOT NULL, relative_path TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL, modified_ns INTEGER NOT NULL,
                    sha256 TEXT
                );
                CREATE INDEX IF NOT EXISTS model_kind_value ON model_files(kind,value);
                CREATE INDEX IF NOT EXISTS model_hash ON model_files(sha256);
                CREATE TABLE IF NOT EXISTS model_metadata (
                    sha256 TEXT NOT NULL, owner TEXT NOT NULL, payload TEXT NOT NULL,
                    PRIMARY KEY(sha256,owner)
                );
                CREATE TABLE IF NOT EXISTS local_metadata (
                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, payload TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS thumbnail_choices (
                    id TEXT PRIMARY KEY, payload TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS detector_associations (
                    value TEXT PRIMARY KEY, asset TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS model_architectures (
                    id TEXT PRIMARY KEY, size_bytes INTEGER NOT NULL,
                    modified_ns INTEGER NOT NULL, signature TEXT NOT NULL,
                    base TEXT, comfy_class TEXT
                );
                CREATE TABLE IF NOT EXISTS model_content_overrides (
                    sha256 TEXT PRIMARY KEY, nsfw INTEGER NOT NULL CHECK(nsfw IN (0,1))
                );
                CREATE TABLE IF NOT EXISTS model_download_preferences (
                    id INTEGER PRIMARY KEY CHECK(id=1), pattern TEXT NOT NULL
                );
                PRAGMA user_version=1;
            """
            for statement in schema.split(";"):
                if statement.strip():
                    connection.execute(statement)
            migrate_provider_metadata(connection)

    @contextmanager
    def connect(self) -> Iterator[sqlite3.Connection]:
        """Bound lock waits, roll back incomplete writes and release every database handle."""
        connection = sqlite3.connect(self._path, timeout=30)
        connection.row_factory = sqlite3.Row
        try:
            with connection:
                yield connection
        finally:
            connection.close()

    def advance_revision(self, connection: sqlite3.Connection) -> None:
        """Advance the shared token inside the same transaction as its catalog mutation."""
        connection.execute("UPDATE catalog_state SET revision=revision+1 WHERE id=1")

    def revision(self, connection: sqlite3.Connection) -> int:
        """Validate committed state at the database boundary."""
        row = connection.execute(
            "SELECT revision FROM catalog_state WHERE id=1"
        ).fetchone()
        if row is None:
            raise ValueError("Model catalog revision is missing.")
        return catalog_integer(row, "revision")

    def invalidate(self) -> None:
        """Commit an explicit cache invalidation even when visible catalog rows are unchanged."""
        with self.connect() as connection:
            self.advance_revision(connection)
