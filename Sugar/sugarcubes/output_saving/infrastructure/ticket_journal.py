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
"""Persist server-issued saving tickets and atomic per-run numbering across sink instances."""

from __future__ import annotations

from collections.abc import Callable, Iterator
from contextlib import contextmanager
import json
from pathlib import Path
import sqlite3
import uuid

from ..domain.run_codec import decode_run, decode_source, encode_run, encode_source
from ..domain.run_plan import SavingRun, SavingSource, SavingTicket


class SavingTicketJournal:
    """Own durable immutable run policies, source tickets and transactional counters."""

    def __init__(self, path: Path) -> None:
        """Create one composition-owned journal without discovering host globals."""
        self._path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with self._connection() as connection:
            connection.executescript(
                "CREATE TABLE IF NOT EXISTS journal_schema (version INTEGER NOT NULL);"
                "INSERT INTO journal_schema(version) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM journal_schema);"
            )
            versions = connection.execute(
                "SELECT version FROM journal_schema"
            ).fetchall()
            if versions != [(1,)]:
                raise ValueError("Unsupported output saving journal version.")
            connection.executescript(
                "CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, payload TEXT NOT NULL, number INTEGER);"
                "CREATE TABLE IF NOT EXISTS tickets (id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id), node_id TEXT NOT NULL, payload TEXT NOT NULL, image_index INTEGER NOT NULL DEFAULT 0, UNIQUE(run_id,node_id));"
                "CREATE TABLE IF NOT EXISTS counters (bucket TEXT PRIMARY KEY, number INTEGER NOT NULL);"
            )

    def issue_run(
        self, run: SavingRun, sources: tuple[SavingSource, ...]
    ) -> tuple[str, ...]:
        """Atomically capture all sources, rejecting reused run identities and incomplete transactions."""
        payload = encode_run(run)
        decode_run(payload)
        source_payloads: list[tuple[str, str, str, str]] = []
        for source in sources:
            source_payload = encode_source(source)
            decode_source(source_payload)
            source_payloads.append(
                (
                    str(uuid.uuid4()),
                    run.run_id,
                    source.node_id,
                    json.dumps(source_payload, ensure_ascii=False),
                )
            )
        with self._connection() as connection:
            connection.execute(
                "INSERT INTO runs(id,payload) VALUES (?,?)",
                (run.run_id, json.dumps(payload, ensure_ascii=False)),
            )
            connection.executemany(
                "INSERT INTO tickets(id,run_id,node_id,payload) VALUES (?,?,?,?)",
                source_payloads,
            )
        return tuple(item[0] for item in source_payloads)

    def read_ticket(self, ticket_id: str) -> SavingTicket:
        """Resolve immutable policy only for a server-issued ticket with valid journal bytes."""
        with self._connection() as connection:
            row = connection.execute(
                "SELECT runs.payload,tickets.payload FROM tickets JOIN runs ON runs.id=tickets.run_id WHERE tickets.id=?",
                (ticket_id,),
            ).fetchone()
        if row is None:
            raise ValueError("The output saving ticket is unknown or expired.")
        run_payload: object = row[0]
        source_payload: object = row[1]
        if not isinstance(run_payload, str) or not isinstance(source_payload, str):
            raise ValueError("The output saving ticket journal contains invalid data.")
        run_value: object = json.loads(run_payload)
        source_value: object = json.loads(source_payload)
        return SavingTicket(
            ticket_id, decode_run(run_value), decode_source(source_value)
        )

    def next_source_index(self, ticket_id: str) -> int:
        """Allocate one-based source image positions across list and batch sink calls."""
        with self._connection() as connection:
            row = connection.execute(
                "UPDATE tickets SET image_index=image_index+1 WHERE id=? RETURNING image_index",
                (ticket_id,),
            ).fetchone()
        if row is None:
            raise ValueError("The output saving ticket is unknown or expired.")
        return _positive_integer(row[0])

    def discard_run(self, run_id: str) -> None:
        """Retire completed or rejected run snapshots without deleting saved artifacts or numbering state."""
        with self._connection() as connection:
            connection.execute("DELETE FROM tickets WHERE run_id=?", (run_id,))
            connection.execute("DELETE FROM runs WHERE id=?", (run_id,))

    def retain_runs(self, live_ids: frozenset[str]) -> None:
        """Retire interrupted prior-host snapshots without changing saved files or counters."""
        with self._connection() as connection:
            rows = connection.execute("SELECT id FROM runs").fetchall()
            stale: list[tuple[str]] = []
            for row in rows:
                identity: object = row[0]
                if not isinstance(identity, str):
                    raise ValueError(
                        "Output saving journal contains an invalid run identity."
                    )
                if identity not in live_ids:
                    stale.append((identity,))
            connection.executemany("DELETE FROM tickets WHERE run_id=?", stale)
            connection.executemany("DELETE FROM runs WHERE id=?", stale)

    def run_number(
        self, run_id: str, bucket: str, initial_number: Callable[[], int]
    ) -> int:
        """Share one committed run number across all sources and allocate it on first saved output."""
        with self._connection() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT number FROM runs WHERE id=?", (run_id,)
            ).fetchone()
            if row is None:
                raise ValueError("The output saving generation is unknown.")
            number: object = row[0]
            if number is not None:
                return _positive_integer(number)
            allocated = self._counter(connection, f"run:{bucket}", initial_number)
            connection.execute(
                "UPDATE runs SET number=? WHERE id=?", (allocated, run_id)
            )
            return allocated

    def image_number(self, bucket: str, initial_number: Callable[[], int]) -> int:
        """Reserve a folder-wide image ordinal without retaining an unbounded in-memory set."""
        with self._connection() as connection:
            connection.execute("BEGIN IMMEDIATE")
            return self._counter(connection, f"image:{bucket}", initial_number)

    @staticmethod
    def _counter(
        connection: sqlite3.Connection, bucket: str, initial_number: Callable[[], int]
    ) -> int:
        """Scan existing files only for a previously unseen persistent counter namespace."""
        row = connection.execute(
            "SELECT number FROM counters WHERE bucket=?", (bucket,)
        ).fetchone()
        number = (
            _positive_integer(initial_number())
            if row is None
            else _positive_integer(row[0]) + 1
        )
        connection.execute(
            "INSERT INTO counters(bucket,number) VALUES (?,?) ON CONFLICT(bucket) DO UPDATE SET number=excluded.number",
            (bucket, number),
        )
        return number

    @contextmanager
    def _connection(self) -> Iterator[sqlite3.Connection]:
        """Close every connection and roll back failed writes while preserving original exceptions."""
        connection = sqlite3.connect(self._path, timeout=30)
        try:
            connection.execute("PRAGMA foreign_keys=ON")
            with connection:
                yield connection
        finally:
            connection.close()


def _positive_integer(value: object) -> int:
    """Reject corrupt or boolean counter values at the SQLite adapter boundary."""
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise ValueError("Output saving journal counter must be a positive integer.")
    return value
