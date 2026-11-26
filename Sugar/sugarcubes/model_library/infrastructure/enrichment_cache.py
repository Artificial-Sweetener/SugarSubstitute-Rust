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
"""Persist confirmed provider decisions separately from retry leases and user metadata."""

from __future__ import annotations

from contextlib import contextmanager
from collections.abc import Iterator
from dataclasses import replace
import json
import logging
from pathlib import Path
import sqlite3

from .library_sqlite import library_database
from threading import RLock

from ..domain.enrichment_freshness import ModelEnrichmentRecord
from ..domain.models import ModelEvidence
from .metadata_codec import decode_metadata, encode_metadata, normalized_hash
from .enrichment_ownership_codec import (
    decode_enrichment_providers,
    legacy_enrichment_providers,
    enrichment_thumbnail_origin,
)

_LOGGER = logging.getLogger(__name__)


def enrichment_evidence_signature(evidence: ModelEvidence) -> str:
    """Retain every host selection and stat field that can invalidate a provider decision."""
    return json.dumps(
        (
            evidence.kind,
            evidence.value,
            evidence.root_id,
            evidence.relative_path,
            evidence.size_bytes,
            evidence.modified_ns,
        ),
        separators=(",", ":"),
    )


class SqliteModelEnrichmentCache:
    """Keep durable match and miss evidence in the configured model-root namespace."""

    def __init__(self, path: Path) -> None:
        """Initialize an additive cache without altering existing catalog or retry data."""
        self._path = path
        self._lock = RLock()
        self._rows: dict[str, tuple[str, ModelEnrichmentRecord | None]] | None = None
        path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            version = connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, 1):
                raise ValueError(
                    "Unsupported enrichment cache schema; original data was preserved."
                )
            connection.execute("""CREATE TABLE IF NOT EXISTS enrichment_records (
                id TEXT PRIMARY KEY, signature TEXT NOT NULL, sha256 TEXT NOT NULL,
                policy TEXT NOT NULL, metadata TEXT, local_preview INTEGER NOT NULL,
                provider_scope TEXT NOT NULL DEFAULT ''
            )""")
            connection.execute("PRAGMA user_version=1")
            columns = {
                row[1]
                for row in connection.execute("PRAGMA table_info(enrichment_records)")
            }
            if "provider_scope" not in columns:
                connection.execute(
                    "ALTER TABLE enrichment_records ADD COLUMN provider_scope TEXT NOT NULL DEFAULT ''"
                )
            if "thumbnail_origin" not in columns:
                connection.execute(
                    "ALTER TABLE enrichment_records ADD COLUMN thumbnail_origin TEXT"
                )
            if "providers" not in columns:
                connection.execute(
                    "ALTER TABLE enrichment_records ADD COLUMN providers TEXT NOT NULL DEFAULT '[]'"
                )
                for identity, scope in connection.execute(
                    "SELECT id,provider_scope FROM enrichment_records"
                ).fetchall():
                    connection.execute(
                        "UPDATE enrichment_records SET providers=? WHERE id=?",
                        (json.dumps(legacy_enrichment_providers(scope)), identity),
                    )
            if "thumbnail_origin" not in columns:
                for row in connection.execute(
                    "SELECT id,sha256,policy,metadata,local_preview,provider_scope,thumbnail_origin,providers FROM enrichment_records WHERE metadata IS NOT NULL"
                ).fetchall():
                    record = _enrichment_record(*row[1:])
                    if record.thumbnail_origin is not None:
                        connection.execute(
                            "UPDATE enrichment_records SET thumbnail_origin=? WHERE id=?",
                            (record.thumbnail_origin, row[0]),
                        )

    def read(self, evidence: ModelEvidence) -> ModelEnrichmentRecord | None:
        """Reject changed evidence and validate cached content at the SQLite boundary.

        Every model is read each time the catalog is built, so the rows are held in memory
        between writes instead of being queried one model at a time.
        """
        with self._lock:
            rows = self._rows
            if rows is None:
                with self._connect() as connection:
                    rows = {
                        str(row[0]): (str(row[1]), self._decoded(str(row[0]), row[2:]))
                        for row in connection.execute(
                            "SELECT id,signature,sha256,policy,metadata,local_preview,provider_scope,thumbnail_origin,providers FROM enrichment_records"
                        )
                    }
                self._rows = rows
        stored = rows.get(evidence.location_id)
        if stored is None or stored[0] != enrichment_evidence_signature(evidence):
            return None
        return stored[1]

    def _decoded(
        self, identity: str, row: tuple[object, ...]
    ) -> ModelEnrichmentRecord | None:
        """Validate one stored decision; an invalid one is reconsidered as if it were absent."""
        try:
            return _enrichment_record(*row)
        except ValueError as error:
            _LOGGER.warning(
                "Stored model enrichment evidence is invalid; reconsidering provider freshness",
                extra={"model_id": identity},
                exc_info=error,
            )
            return None

    def save(self, evidence: ModelEvidence, record: ModelEnrichmentRecord) -> None:
        """Commit exact file and provider evidence without replacing deliberate presentation."""
        if not record.policy:
            raise ValueError("Model enrichment policy must be present.")
        payload = (
            encode_metadata(record.metadata) if record.metadata is not None else None
        )
        origin = enrichment_thumbnail_origin(
            record.thumbnail_origin, record.metadata, record.local_preview
        )
        providers = decode_enrichment_providers(json.dumps(record.providers))
        signature = enrichment_evidence_signature(evidence)
        stored: tuple[object, ...] = (
            normalized_hash(record.sha256),
            record.policy,
            payload,
            int(record.local_preview),
            record.provider_scope,
            origin,
            json.dumps(providers),
        )
        with self._lock:
            held = self._rows
            with self._connect() as connection:
                connection.execute(
                    "INSERT OR REPLACE INTO enrichment_records VALUES (?,?,?,?,?,?,?,?,?)",
                    (evidence.location_id, signature, *stored),
                )
            # One decision changed: the held rows take it in place of being read again whole.
            if held is not None:
                held[evidence.location_id] = (
                    signature,
                    self._decoded(evidence.location_id, stored),
                )
                self._rows = held

    def clear_thumbnails(self, origin: str) -> None:
        """Keep confirmed decisions fresh while stripping only the selected provider's images."""
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            for identity, payload in connection.execute(
                "SELECT id,metadata FROM enrichment_records WHERE thumbnail_origin=?",
                (origin,),
            ).fetchall():
                metadata = decode_metadata(payload)
                connection.execute(
                    "UPDATE enrichment_records SET metadata=?,thumbnail_origin=NULL WHERE id=?",
                    (
                        encode_metadata(
                            replace(
                                metadata,
                                thumbnails=(),
                                thumbnail_tier=None,
                                previews=(),
                            )
                        ),
                        identity,
                    ),
                )

    def clear_provider(self, provider: str) -> None:
        """Invalidate the selected provider's positive and negative decisions without touching other scopes."""
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            for identity, payload in connection.execute(
                "SELECT id,providers FROM enrichment_records"
            ).fetchall():
                if provider in decode_enrichment_providers(payload):
                    connection.execute(
                        "DELETE FROM enrichment_records WHERE id=?", (identity,)
                    )

    def former_policies(self) -> dict[str, str]:
        """Name, per file content, the thumbnail setting its stored decision was made under."""
        with self._connect() as connection:
            return {
                str(row[0]): str(row[1])
                for row in connection.execute(
                    "SELECT sha256,policy FROM enrichment_records"
                )
            }

    @contextmanager
    def _connect(self) -> Iterator[sqlite3.Connection]:
        """Commit bounded cache transactions, drop held rows and release every SQLite handle."""
        with library_database(self._path, timeout=5).transaction() as connection:
            before = connection.total_changes
            try:
                yield connection
            finally:
                # Any write may have changed a held row; a save puts its own row back.
                if connection.total_changes != before:
                    with self._lock:
                        self._rows = None


def _enrichment_record(
    sha256: object,
    policy: object,
    metadata: object,
    local_preview: object,
    provider_scope: object,
    thumbnail_origin: object,
    providers: object,
) -> ModelEnrichmentRecord:
    """Narrow persisted cache values before they reach the freshness policy owner."""
    if (
        not isinstance(sha256, str)
        or not isinstance(policy, str)
        or not policy
        or (metadata is not None and not isinstance(metadata, str))
        or not isinstance(local_preview, int)
        or local_preview not in (0, 1)
        or not isinstance(provider_scope, str)
    ):
        raise ValueError("Invalid stored model enrichment evidence.")
    decoded = decode_metadata(metadata) if metadata is not None else None
    return ModelEnrichmentRecord(
        normalized_hash(sha256),
        policy,
        decoded,
        provider_scope,
        bool(local_preview),
        enrichment_thumbnail_origin(thumbnail_origin, decoded, bool(local_preview)),
        decode_enrichment_providers(providers),
    )
