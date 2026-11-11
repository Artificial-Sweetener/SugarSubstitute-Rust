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
"""Store shared non-secret provider preferences beneath the configured model folder."""

from __future__ import annotations

from collections.abc import Callable, Iterator
from contextlib import contextmanager
import json
from pathlib import Path
import sqlite3

from ..domain.provider_preferences import ModelProviderPreferences
from .provider_preference_codec import (
    decode_provider_preferences,
    provider_preferences_payload,
)


class SqliteModelProviderPreferences:
    """Merge browser and host settings in one atomic SQLite transaction."""

    def __init__(self, path: Path) -> None:
        """Initialize or migrate complete settings atomically while preserving saved choices."""
        self._path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as connection:
            connection.execute(
                "CREATE TABLE IF NOT EXISTS preferences (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL)"
            )
            connection.execute(
                "INSERT OR IGNORE INTO preferences VALUES (1,?)",
                (self._encode(ModelProviderPreferences()),),
            )
            connection.execute(
                "UPDATE preferences SET payload=? WHERE id=1",
                (self._encode(self._read(connection)),),
            )

    def load(self) -> ModelProviderPreferences:
        """Read a validated immutable snapshot without caching consumer-local policy."""
        with self._connect() as connection:
            return self._read(connection)

    def change(
        self, update: Callable[[ModelProviderPreferences], ModelProviderPreferences]
    ) -> ModelProviderPreferences:
        """Merge against the latest row so independent setting edits cannot overwrite each other."""
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            current = update(self._read(connection))
            connection.execute(
                "UPDATE preferences SET payload=? WHERE id=1", (self._encode(current),)
            )
            return current

    @staticmethod
    def _encode(value: ModelProviderPreferences) -> str:
        """Store only policy fields under the repository's schema ownership."""
        payload = provider_preferences_payload(value)
        payload.pop("schemaVersion")
        return json.dumps(payload, separators=(",", ":"))

    @staticmethod
    def _read(connection: sqlite3.Connection) -> ModelProviderPreferences:
        """Narrow dynamic SQLite and JSON values before returning domain policy."""
        row: object = connection.execute(
            "SELECT payload FROM preferences WHERE id=1"
        ).fetchone()
        if not isinstance(row, tuple) or len(row) != 1 or not isinstance(row[0], str):
            raise ValueError("Stored provider preferences are unavailable.")
        payload: object = json.loads(row[0])
        return decode_provider_preferences(payload)

    @contextmanager
    def _connect(self) -> Iterator[sqlite3.Connection]:
        """Commit bounded transactions and release handles after both success and failure."""
        connection = sqlite3.connect(self._path, timeout=5)
        try:
            with connection:
                yield connection
        finally:
            connection.close()
