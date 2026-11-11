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
"""Persist successful release checks beneath the shared model library root."""

from __future__ import annotations

from contextlib import closing
from pathlib import Path
import sqlite3

from ..application.release_history import ReleaseHistorySnapshot
from .release_history_codec import decode_history, encode_history


class SqliteReleaseHistory:
    """Retain daily freshness across browser reloads and Comfy restarts."""

    def __init__(self, path: Path) -> None:
        """Create a dedicated cache without modifying catalog or workflow usage state."""
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with closing(sqlite3.connect(path, timeout=5)) as connection, connection:
            connection.execute(
                "CREATE TABLE IF NOT EXISTS release_history (key TEXT PRIMARY KEY, payload TEXT NOT NULL)"
            )

    def read(self, key: str) -> ReleaseHistorySnapshot | None:
        """Validate cached provider evidence at the storage boundary."""
        with closing(sqlite3.connect(self.path, timeout=5)) as connection:
            row = connection.execute(
                "SELECT payload FROM release_history WHERE key=?", (key,)
            ).fetchone()
        if row is None:
            return None
        payload: object = row[0]
        if not isinstance(payload, str):
            raise ValueError("Invalid cached release history.")
        return decode_history(payload)

    def save(self, key: str, snapshot: ReleaseHistorySnapshot) -> None:
        """Commit one validated successful response, leaving earlier history intact on failure."""
        payload = encode_history(snapshot)
        decode_history(payload)
        with closing(sqlite3.connect(self.path, timeout=5)) as connection, connection:
            connection.execute(
                "INSERT INTO release_history (key,payload) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload",
                (key, payload),
            )

    def clear(self) -> None:
        """Remove disposable provider history while retaining user update decisions."""
        with closing(sqlite3.connect(self.path, timeout=5)) as connection, connection:
            connection.execute("DELETE FROM release_history")
