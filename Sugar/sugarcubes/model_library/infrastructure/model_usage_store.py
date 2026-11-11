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
"""Persist update consent and usage atomically beneath the shared model root."""

from __future__ import annotations

from collections.abc import Callable
from contextlib import closing
from pathlib import Path
import sqlite3

from ..domain.model_updates import ModelUpdateState
from .model_usage_codec import decode_usage, encode_usage


class SqliteModelUsageRepository:
    """Separate durable user decisions from disposable provider/cache state."""

    def __init__(self, path: Path) -> None:
        """Create a single-state transactional store without touching model weights."""
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with closing(sqlite3.connect(path, timeout=5)) as connection, connection:
            connection.execute(
                "CREATE TABLE IF NOT EXISTS model_updates (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL)"
            )

    def load(self) -> ModelUpdateState:
        """Read saved preferences or the enabled domain default for a new store."""
        with closing(sqlite3.connect(self.path, timeout=5)) as connection:
            return self._read(connection)

    def change(
        self, transition: Callable[[ModelUpdateState], ModelUpdateState]
    ) -> ModelUpdateState:
        """Serialize concurrent state transitions and roll back failed validation or policy."""
        with closing(sqlite3.connect(self.path, timeout=5)) as connection, connection:
            connection.execute("BEGIN IMMEDIATE")
            state = transition(self._read(connection))
            payload = encode_usage(state)
            decode_usage(payload)
            connection.execute(
                "INSERT INTO model_updates (id,payload) VALUES (1,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
                (payload,),
            )
            return state

    @staticmethod
    def _read(connection: sqlite3.Connection) -> ModelUpdateState:
        """Validate SQLite's dynamic row values at the persistence boundary."""
        row = connection.execute(
            "SELECT payload FROM model_updates WHERE id=1"
        ).fetchone()
        if row is None:
            return ModelUpdateState()
        payload: object = row[0]
        if not isinstance(payload, str):
            raise ValueError("Invalid persisted model update state.")
        return decode_usage(payload)
