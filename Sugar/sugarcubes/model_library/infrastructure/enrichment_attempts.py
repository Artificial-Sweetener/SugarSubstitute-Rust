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
"""Persist enrichment leases and provider retry deadlines beneath the shared model root."""

from __future__ import annotations

from collections.abc import Iterator, Mapping
from contextlib import contextmanager
from pathlib import Path
import json
import sqlite3

from .library_sqlite import library_database

from ..domain.models import ModelEvidence
from .metadata_codec import normalized_hash
from .enrichment_cache import enrichment_evidence_signature
from .enrichment_ownership_codec import (
    decode_enrichment_providers,
    legacy_enrichment_providers,
)


class SqliteEnrichmentAttempts:
    """Keep retry state separate from catalog and thumbnail presentation ownership."""

    def __init__(self, path: Path) -> None:
        """Prepare bounded SQLite transactions in the library's versioned namespace."""
        self._path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as connection:
            connection.executescript("""
                CREATE TABLE IF NOT EXISTS file_attempts (
                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, status TEXT NOT NULL,
                    retry_at REAL NOT NULL
                );
                CREATE TABLE IF NOT EXISTS content_attempts (
                    sha256 TEXT NOT NULL, status TEXT NOT NULL, retry_at REAL NOT NULL,
                    policy TEXT NOT NULL DEFAULT '', PRIMARY KEY(sha256,policy)
                );
                CREATE TABLE IF NOT EXISTS thumbnail_attempts (
                    id TEXT PRIMARY KEY, failures INTEGER NOT NULL, retry_at REAL NOT NULL
                );
                CREATE TABLE IF NOT EXISTS card_repairs (sha256 TEXT PRIMARY KEY);
            """)
            columns = {
                row[1]
                for row in connection.execute("PRAGMA table_info(content_attempts)")
            }
            if "policy" not in columns:
                connection.execute(
                    "ALTER TABLE content_attempts ADD COLUMN policy TEXT NOT NULL DEFAULT ''"
                )
            primary = {
                row[1]: row[5]
                for row in connection.execute("PRAGMA table_info(content_attempts)")
            }
            if primary.get("policy") == 0:
                connection.executescript("""
                    BEGIN IMMEDIATE;
                    ALTER TABLE content_attempts RENAME TO previous_content_attempts;
                    CREATE TABLE content_attempts (
                        sha256 TEXT NOT NULL, status TEXT NOT NULL, retry_at REAL NOT NULL,
                        policy TEXT NOT NULL, PRIMARY KEY(sha256,policy)
                    );
                    INSERT INTO content_attempts SELECT sha256,status,retry_at,policy FROM previous_content_attempts;
                    DROP TABLE previous_content_attempts;
                    COMMIT;
                """)
            for table, policy_column in (
                ("file_attempts", "signature"),
                ("content_attempts", "policy"),
            ):
                fields = {
                    row[1] for row in connection.execute(f"PRAGMA table_info({table})")
                }
                if "providers" not in fields:
                    connection.execute(
                        f"ALTER TABLE {table} ADD COLUMN providers TEXT NOT NULL DEFAULT '[]'"
                    )
                    for row_id, identity in connection.execute(
                        f"SELECT rowid,{policy_column} FROM {table}"
                    ).fetchall():
                        owners = legacy_enrichment_providers(identity.split("|", 1)[0])
                        connection.execute(
                            f"UPDATE {table} SET providers=? WHERE rowid=?",
                            (json.dumps(owners), row_id),
                        )

            if "preview_failures" not in {
                row[1]
                for row in connection.execute("PRAGMA table_info(content_attempts)")
            }:
                connection.execute(
                    "ALTER TABLE content_attempts ADD COLUMN preview_failures INTEGER NOT NULL DEFAULT 0"
                )

    def reserve(
        self,
        evidence: ModelEvidence,
        now: float,
        *,
        policy: str,
        providers: tuple[str, ...],
    ) -> bool:
        """Lease one unchanged physical file across concurrent library consumers."""
        signature = f"{policy}:{enrichment_evidence_signature(evidence)}"
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT signature,retry_at FROM file_attempts WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            if row is not None and row[0] == signature and float(row[1]) > now:
                return False
            connection.execute(
                "INSERT OR REPLACE INTO file_attempts VALUES (?,?,?,?,?)",
                (
                    evidence.location_id,
                    signature,
                    "running",
                    now + 900,
                    json.dumps(providers),
                ),
            )
        return True

    def content_retry_at(self, sha256: str, *, policy: str) -> float:
        """Reuse the exact retry deadline without extending another file's failure cooldown."""
        with self._connect() as connection:
            row = connection.execute(
                "SELECT retry_at FROM content_attempts WHERE sha256=? AND policy=?",
                (normalized_hash(sha256), policy),
            ).fetchone()
            return 0 if row is None else float(row[0])

    def finish(
        self,
        evidence: ModelEvidence,
        sha256: str | None,
        status: str,
        retry_at: float,
        *,
        policy: str,
        providers: tuple[str, ...],
    ) -> None:
        """Record the old file's result without overwriting a lease for changed evidence."""
        signature = f"{policy}:{enrichment_evidence_signature(evidence)}"
        with self._connect() as connection:
            connection.execute(
                "UPDATE file_attempts SET status=?,retry_at=? WHERE id=? AND signature=?",
                (status, retry_at, evidence.location_id, signature),
            )
            if sha256 is not None:
                connection.execute(
                    "INSERT OR REPLACE INTO content_attempts VALUES (?,?,?,?,?,0)",
                    (
                        normalized_hash(sha256),
                        status,
                        retry_at,
                        policy,
                        json.dumps(providers),
                    ),
                )

    def release_abandoned(self) -> int:
        """Release every lease still marked running; return how many there were.

        A lease lasts fifteen minutes so that two consumers never look one file up at once.
        A host that was killed mid-lookup leaves its leases behind, and nothing is running
        them. The library calls this once as it starts, before it queues any work, so a model
        interrupted by a crash is looked up again at once and not a quarter of an hour later.
        """
        with self._connect() as connection:
            return connection.execute(
                "UPDATE file_attempts SET status='abandoned',retry_at=0 WHERE status='running'"
            ).rowcount

    def failed_lookups(self) -> frozenset[str]:
        """Name files whose last lookup failed, so their cards can say a retry is coming."""
        with self._connect() as connection:
            return frozenset(
                str(row[0])
                for row in connection.execute(
                    "SELECT id FROM file_attempts WHERE status='failed'"
                )
            )

    def thumbnail_failures(self) -> Mapping[str, tuple[int, float]]:
        """Read every remembered thumbnail failure with the moment its retry is due."""
        with self._connect() as connection:
            return {
                str(row[0]): (int(row[1]), float(row[2]))
                for row in connection.execute(
                    "SELECT id,failures,retry_at FROM thumbnail_attempts"
                )
            }

    def thumbnail_failed(self, model_id: str, failures: int, retry_at: float) -> None:
        """Remember one more failed fetch for a model and when it may be tried again."""
        with self._connect() as connection:
            connection.execute(
                "INSERT OR REPLACE INTO thumbnail_attempts VALUES (?,?,?)",
                (normalized_hash(model_id), failures, retry_at),
            )

    def thumbnail_fetched(self, model_id: str) -> None:
        """Forget a model's thumbnail failures once its image arrived."""
        with self._connect() as connection:
            connection.execute(
                "DELETE FROM thumbnail_attempts WHERE id=?",
                (normalized_hash(model_id),),
            )

    def repaired(self) -> frozenset[str]:
        """Name the file contents whose stored card already had its one repair."""
        with self._connect() as connection:
            return frozenset(
                str(row[0])
                for row in connection.execute("SELECT sha256 FROM card_repairs")
            )

    def mark_repaired(self, sha256: str) -> None:
        """Record a completed repair so the provider is never asked for that offer again."""
        with self._connect() as connection:
            connection.execute(
                "INSERT OR IGNORE INTO card_repairs VALUES (?)",
                (normalized_hash(sha256),),
            )

    def clear_provider(self, provider: str) -> None:
        """Release both file leases and content cooldowns owned by the selected provider."""
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            for table in ("file_attempts", "content_attempts"):
                for row_id, payload in connection.execute(
                    f"SELECT rowid,providers FROM {table}"
                ).fetchall():
                    if provider in decode_enrichment_providers(payload):
                        connection.execute(
                            f"DELETE FROM {table} WHERE rowid=?", (row_id,)
                        )

    @contextmanager
    def _connect(self) -> Iterator[sqlite3.Connection]:
        """Commit small retry transactions and release file handles even after failures."""
        with library_database(self._path, timeout=5).transaction() as connection:
            yield connection
