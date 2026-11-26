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

from collections.abc import Callable, Iterable, Iterator
from contextlib import contextmanager
from pathlib import Path
import sqlite3
from threading import local

from .library_sqlite import library_database

from .catalog_row_projection import catalog_integer, catalog_text
from .provider_metadata_migration import migrate_provider_metadata

# How many revisions back a consumer can be and still be told which models changed.
_REMEMBERED_REVISIONS = 1024
# A change touching more models than this is recorded as touching everything.
_LARGEST_NAMED_CHANGE = 256
# Marks in the change log: a change to everything, and a change no model shows.
_EVERYTHING = "*"
_NOTHING = ""


class ModelCatalogDatabase:
    """Share one versioned catalog namespace among focused persistence adapters."""

    def __init__(self, model_root: Path) -> None:
        """Initialize the established schema without replacing unsupported user data."""
        # What the operation running on this thread has done so far.
        self._operation = local()
        self._commit_listener: Callable[[], None] | None = None
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
                CREATE TABLE IF NOT EXISTS catalog_changes (
                    revision INTEGER NOT NULL, id TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS catalog_change_revision
                    ON catalog_changes(revision);
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
        """Run one catalog operation: commit at its end, roll back if it fails.

        An operation that advanced the revision is announced once, after its commit.
        """
        database = library_database(self._path, timeout=30, rows=True)
        outermost = not getattr(self._operation, "open", False)
        self._operation.open = True
        try:
            with database.transaction() as connection:
                yield connection
        except BaseException:
            if outermost:
                self._operation.advanced = False
            raise
        finally:
            if outermost:
                self._operation.open = False
        if outermost and getattr(self._operation, "advanced", False):
            self._operation.advanced = False
            if self._commit_listener is not None:
                self._commit_listener()

    def watch_commits(self, listener: Callable[[], None]) -> None:
        """Report every committed revision advance to the one publication owner.

        The listener runs on whichever thread committed, after the transaction is durable.
        """
        self._commit_listener = listener

    def advance_revision(
        self, connection: sqlite3.Connection, changed: Iterable[str] | None = None
    ) -> None:
        """Advance the shared token and record which models the change touched.

        Both are written in the same transaction as the change itself, so a consumer holding
        an earlier revision can be told exactly which models to read again. ``changed`` names
        those models by catalog identity. None says the change cannot be named model by
        model, and a consumer reads the whole catalog again.
        """
        self._operation.advanced = True
        connection.execute("UPDATE catalog_state SET revision=revision+1 WHERE id=1")
        revision = self.revision(connection)
        named = None if changed is None else tuple(dict.fromkeys(changed))
        if named is None or len(named) > _LARGEST_NAMED_CHANGE:
            named = (_EVERYTHING,)
        connection.executemany(
            "INSERT INTO catalog_changes VALUES (?,?)",
            ((revision, identity) for identity in (named or (_NOTHING,))),
        )
        connection.execute(
            "DELETE FROM catalog_changes WHERE revision<=?",
            (revision - _REMEMBERED_REVISIONS,),
        )

    def advance_for_content(self, connection: sqlite3.Connection, sha256: str) -> None:
        """Advance the token for a change to what is known about one file content.

        Every catalog entry with that content shows the change; copies of a file share it.
        """
        self.advance_revision(
            connection,
            tuple(
                catalog_text(row, "id")
                for row in connection.execute(
                    "SELECT id FROM model_files WHERE sha256=?", (sha256,)
                )
            ),
        )

    def changes_since(
        self, connection: sqlite3.Connection, held: int
    ) -> tuple[int, frozenset[str] | None]:
        """Return the current revision and the models changed after a held one.

        The models are None when they cannot be named: a change touched everything, or the
        held revision is older than the log remembers.
        """
        revision = self.revision(connection)
        if held >= revision:
            return revision, frozenset()
        rows = connection.execute(
            "SELECT revision,id FROM catalog_changes WHERE revision>?", (held,)
        ).fetchall()
        recorded = {catalog_integer(row, "revision") for row in rows}
        identities = {catalog_text(row, "id") for row in rows}
        if len(recorded) != revision - held or _EVERYTHING in identities:
            return revision, None
        return revision, frozenset(identities - {_NOTHING})

    def revision(self, connection: sqlite3.Connection) -> int:
        """Validate committed state at the database boundary."""
        row = connection.execute(
            "SELECT revision FROM catalog_state WHERE id=1"
        ).fetchone()
        if row is None:
            raise ValueError("Model catalog revision is missing.")
        return catalog_integer(row, "revision")

    def invalidate(self, changed: Iterable[str] | None = None) -> None:
        """Announce a change that no catalog row holds, for the named models or for all of them."""
        with self.connect() as connection:
            self.advance_revision(connection, changed)
