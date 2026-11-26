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
"""Open each model-library database once, and commit to it without waiting on the disk.

Identifying one model writes to several of these databases about ten times. Two things made
each of those writes cost milliseconds, and on a hard drive made the drive, not the work, set
how fast a library fills:

- SQLite's default journal flushes the disk on every commit. A write-ahead log keeps each
  commit atomic and the file safe from corruption while flushing only at checkpoints. What a
  power cut can lose is the last few commits, and everything stored here can be learned again.
- Opening a connection costs far more than a commit. Every database here is opened once and
  that one connection is shared, one operation at a time.
"""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from functools import lru_cache
import logging
import os
from pathlib import Path
import sqlite3
from threading import Lock, RLock

_LOGGER = logging.getLogger(__name__)
_REGISTRY_LOCK = Lock()
# The schema versions the library's stores accept; any other is a newer library's database.
_KNOWN_VERSIONS = (0, 1)
_DATABASES: dict[str, LibraryDatabase] = {}


class LibraryDatabase:
    """One held connection to one library database, used by one operation at a time.

    An operation is everything inside one ``transaction()`` block: it commits when the block
    ends and rolls back if the block raises. A block entered inside another on the same thread
    joins the outer one, so the whole of it commits or none of it does.
    """

    def __init__(
        self, path: Path, *, timeout: float, rows: bool, foreign_keys: bool
    ) -> None:
        """Open the database with a write-ahead log where its drive supports one."""
        self.path = path
        self._lock = RLock()
        self._depth = 0
        self._connection: sqlite3.Connection | None = sqlite3.connect(
            path, timeout=timeout, check_same_thread=False
        )
        connection = self._connection
        try:
            if rows:
                connection.row_factory = sqlite3.Row
            self._journal(connection)
            # With a write-ahead log this flushes at checkpoints only; with the default
            # journal SQLite still flushes at the points that keep the file uncorrupted.
            connection.execute("PRAGMA synchronous=NORMAL")
            if foreign_keys:
                connection.execute("PRAGMA foreign_keys=ON")
        except BaseException:
            self.close()
            raise

    def _journal(self, connection: sqlite3.Connection) -> None:
        """Ask for a write-ahead log; a drive that cannot hold one keeps the default journal.

        Some network shares cannot. The default journal is slower and exactly as safe. A
        database written by a newer version of the library is left exactly as it is: its
        store refuses it, and refusing must not change a byte of it.
        """
        version = connection.execute("PRAGMA user_version").fetchone()
        connection.rollback()
        if version is None or version[0] not in _KNOWN_VERSIONS:
            return
        try:
            mode = connection.execute("PRAGMA journal_mode=WAL").fetchone()
        except sqlite3.OperationalError as error:
            mode = (str(error),)
        if mode is None or str(mode[0]).casefold() != "wal":
            _LOGGER.warning(
                "Model library database keeps its default journal. "
                "operation=model_library_journal answer=%s",
                None if mode is None else mode[0],
                extra={
                    "operation": "model_library_journal",
                    "database": self.path.name,
                },
            )

    @contextmanager
    def transaction(self) -> Iterator[sqlite3.Connection]:
        """Run one operation on the held connection: commit at the end, roll back on failure."""
        with self._lock:
            connection = self._connection
            if connection is None:
                raise sqlite3.ProgrammingError("The model library database is closed.")
            self._depth += 1
            try:
                if self._depth > 1:
                    # Joined to the operation already running on this thread.
                    yield connection
                    return
                try:
                    yield connection
                except BaseException:
                    connection.rollback()
                    raise
                connection.commit()
            finally:
                self._depth -= 1

    def close(self) -> None:
        """Release the file; a closed database is opened again by the next store that asks."""
        with self._lock:
            connection, self._connection = self._connection, None
            if connection is not None:
                connection.close()
        with _REGISTRY_LOCK:
            if _DATABASES.get(_key(self.path)) is self:
                del _DATABASES[_key(self.path)]


def library_database(
    path: Path, *, timeout: float, rows: bool = False, foreign_keys: bool = False
) -> LibraryDatabase:
    """Return the one held connection for a database file, opening it on first use.

    Every store of the same file shares it, so two stores never wait on each other's file
    locks and no store pays for opening the file again.
    """
    key = _key(path)
    with _REGISTRY_LOCK:
        database = _DATABASES.get(key)
        if database is None:
            database = LibraryDatabase(
                path, timeout=timeout, rows=rows, foreign_keys=foreign_keys
            )
            _DATABASES[key] = database
        return database


def close_library_databases(root: Path) -> None:
    """Close every held database beneath a folder, when the library that owns it shuts down."""
    prefix = _key(root)
    with _REGISTRY_LOCK:
        held = [
            database
            for key, database in _DATABASES.items()
            if key == prefix or key.startswith(prefix + os.sep)
        ]
    for database in held:
        database.close()


def close_all_library_databases() -> None:
    """Close every held database; for a host that is shutting down and for test isolation."""
    with _REGISTRY_LOCK:
        held = list(_DATABASES.values())
    for database in held:
        database.close()


def _key(path: Path) -> str:
    """Name a database by its resolved location, so every spelling of it shares one connection."""
    return _resolved(str(path))


@lru_cache(maxsize=512)
def _resolved(spelling: str) -> str:
    """Resolve one spelling of a path once; resolving asks the drive, and stores ask per operation."""
    return os.path.normcase(str(Path(spelling).resolve()))
