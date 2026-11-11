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
"""Own versioned model-root catalog storage and atomic evidence updates."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

from ..domain.models import (
    ModelCatalogEntry,
    ModelEvidence,
    ModelMetadata,
    CatalogSnapshot,
    ThumbnailReference,
)
from .metadata_codec import decode_metadata, encode_metadata, normalized_hash
from .catalog_row_projection import catalog_entry, catalog_integer, catalog_text
from .catalog_database import ModelCatalogDatabase
from ..domain.provider_metadata import (
    ProviderMetadataPresentation,
    ProviderMetadataRecord,
)
from .provider_metadata_store import SqliteModelProviderMetadata

_CATALOG_JOIN = """SELECT f.*, u.payload AS user_metadata, l.payload AS local_metadata,
    t.payload AS chosen_thumbnail, a.base AS architecture_base,
    a.comfy_class AS architecture_class, c.nsfw AS nsfw_override
    FROM model_files f
    LEFT JOIN model_metadata u ON u.sha256=f.sha256 AND u.owner='user'
    LEFT JOIN local_metadata l ON l.id=f.id
    LEFT JOIN thumbnail_choices t ON t.id=f.id
    LEFT JOIN model_architectures a ON a.id=f.id AND a.size_bytes=f.size_bytes
        AND a.modified_ns=f.modified_ns
    LEFT JOIN model_content_overrides c ON c.sha256=f.sha256"""


class SqliteModelCatalogRepository:
    """Keep provider caches and user-owned overrides in independent records."""

    def __init__(self, model_root: Path) -> None:
        """Open only SugarCubes' versioned namespace beneath the model folder."""
        self.database = ModelCatalogDatabase(model_root)
        self.provider_cache = SqliteModelProviderMetadata(self.database)

    @property
    def asset_root(self) -> Path:
        """Expose the namespace for the separate prepared-asset owner."""
        return self.database.asset_root

    def reconcile(
        self, entries: Sequence[ModelEvidence], kinds: Sequence[str] | None
    ) -> int:
        """Publish a scan transaction while retaining compatible fingerprints."""
        if kinds is not None and any(entry.kind not in kinds for entry in entries):
            raise ValueError(
                "Scanned model evidence does not match the requested kinds."
            )
        with self.database.connect() as connection:
            existing = {
                catalog_text(row, "id"): row
                for row in connection.execute("SELECT * FROM model_files")
                if kinds is None or catalog_text(row, "kind") in kinds
            }
            changed = False
            seen: set[str] = set()
            for evidence in entries:
                identity = evidence.location_id
                if identity in seen:
                    raise ValueError(
                        "Model scan contains duplicate physical identities."
                    )
                seen.add(identity)
                old = existing.get(identity)
                fields = (
                    evidence.kind,
                    evidence.value,
                    evidence.root_id,
                    evidence.relative_path,
                    evidence.size_bytes,
                    evidence.modified_ns,
                )
                names = (
                    "kind",
                    "value",
                    "root_id",
                    "relative_path",
                    "size_bytes",
                    "modified_ns",
                )
                if old is not None and tuple(old[name] for name in names) == fields:
                    continue
                fingerprint: object = None
                if old is not None and (
                    catalog_integer(old, "size_bytes") == evidence.size_bytes
                    and catalog_integer(old, "modified_ns") == evidence.modified_ns
                ):
                    fingerprint = old["sha256"]
                connection.execute(
                    "INSERT OR REPLACE INTO model_files VALUES (?,?,?,?,?,?,?,?)",
                    (identity, *fields, fingerprint),
                )
                connection.execute("DELETE FROM local_metadata WHERE id=?", (identity,))
                connection.execute(
                    "DELETE FROM model_architectures WHERE id=?", (identity,)
                )
                if fingerprint is None:
                    connection.execute(
                        "DELETE FROM thumbnail_choices WHERE id=?", (identity,)
                    )
                changed = True
            for identity in existing.keys() - seen:
                connection.execute("DELETE FROM model_files WHERE id=?", (identity,))
                connection.execute(
                    "DELETE FROM model_architectures WHERE id=?", (identity,)
                )
                connection.execute("DELETE FROM local_metadata WHERE id=?", (identity,))
                connection.execute(
                    "DELETE FROM thumbnail_choices WHERE id=?", (identity,)
                )
                changed = True
            if changed:
                self.database.advance_revision(connection)
            return self.database.revision(connection)

    def list_entries(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelCatalogEntry, ...]:
        """Join enrichment by content identity without reading image payloads."""
        return self.snapshot(kinds).entries

    def snapshot(self, kinds: Sequence[str] | None = None) -> CatalogSnapshot:
        """Keep returned row data and the client invalidation token transactionally consistent."""
        with self.database.connect() as connection:
            connection.execute("BEGIN")
            revision = self.database.revision(connection)
            providers = self.provider_cache.presentations(connection)
            rows = connection.execute(
                _CATALOG_JOIN + " ORDER BY f.kind, f.value, f.root_id"
            )
            return CatalogSnapshot(
                revision,
                tuple(
                    catalog_entry(
                        row,
                        (
                            providers.get(
                                catalog_text(row, "sha256"),
                                ProviderMetadataPresentation(),
                            )
                            if row["sha256"] is not None
                            else ProviderMetadataPresentation()
                        ),
                    )
                    for row in rows
                    if kinds is None or catalog_text(row, "kind") in kinds
                ),
            )

    def find_entry(self, location_id: str) -> ModelCatalogEntry | None:
        """Resolve one approved identity through the catalog index rather than decoding every model."""
        with self.database.connect() as connection:
            connection.execute("BEGIN")
            row = connection.execute(
                _CATALOG_JOIN + " WHERE f.id=?", (location_id,)
            ).fetchone()
            if row is None:
                return None
            sha256 = catalog_text(row, "sha256") if row["sha256"] is not None else None
            return catalog_entry(
                row, self.provider_cache.presentation(connection, sha256)
            )

    def local_signature(self, location_id: str) -> str | None:
        """Check source-image and sidecar evidence before preparing the same assets again."""
        with self.database.connect() as connection:
            row = connection.execute(
                "SELECT signature FROM local_metadata WHERE id=?", (location_id,)
            ).fetchone()
            return catalog_text(row, "signature") if row is not None else None

    def save_local_metadata(
        self, evidence: ModelEvidence, signature: str, metadata: ModelMetadata
    ) -> bool:
        """Attach unverified local presentation without asserting a trusted content hash."""
        payload = encode_metadata(metadata)
        decode_metadata(payload)
        with self.database.connect() as connection:
            row = connection.execute(
                "SELECT size_bytes,modified_ns FROM model_files WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            if (
                row is None
                or row["size_bytes"] != evidence.size_bytes
                or row["modified_ns"] != evidence.modified_ns
            ):
                return False
            connection.execute(
                "INSERT OR REPLACE INTO local_metadata VALUES (?,?,?)",
                (evidence.location_id, signature, payload),
            )
            self.database.advance_revision(connection)
            return True

    def revision(self) -> int:
        """Read the committed revision shared by every catalog consumer."""
        with self.database.connect() as connection:
            return self.database.revision(connection)

    def detector_associations(self) -> dict[str, str]:
        """Read exact host-value preferences independently from disposable file evidence."""
        with self.database.connect() as connection:
            return {
                catalog_text(row, "value"): catalog_text(row, "asset")
                for row in connection.execute(
                    "SELECT value,asset FROM detector_associations"
                )
            }

    def save_detector_association(self, value: str, asset: str) -> None:
        """Commit a validated preference and its shared invalidation token atomically."""
        with self.database.connect() as connection:
            previous = connection.execute(
                "SELECT asset FROM detector_associations WHERE value=?", (value,)
            ).fetchone()
            if previous is not None and catalog_text(previous, "asset") == asset:
                return
            connection.execute(
                "INSERT INTO detector_associations (value,asset) VALUES (?,?) ON CONFLICT(value) DO UPDATE SET asset=excluded.asset",
                (value, asset),
            )
            self.database.advance_revision(connection)

    def save_thumbnail_choice(
        self, evidence: ModelEvidence, thumbnails: tuple[ThumbnailReference, ...]
    ) -> bool:
        """Retain the chosen image independently of provider metadata and local preview rescans."""
        payload = encode_metadata(ModelMetadata(title="", thumbnails=thumbnails))
        decode_metadata(payload)
        with self.database.connect() as connection:
            row = connection.execute(
                "SELECT size_bytes,modified_ns FROM model_files WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            if (
                row is None
                or row["size_bytes"] != evidence.size_bytes
                or row["modified_ns"] != evidence.modified_ns
            ):
                return False
            connection.execute(
                "INSERT OR REPLACE INTO thumbnail_choices VALUES (?,?)",
                (evidence.location_id, payload),
            )
            self.database.advance_revision(connection)
            return True

    def save_fingerprint(self, evidence: ModelEvidence, sha256: str) -> bool:
        """Reject a hash computed against an obsolete file observation."""
        fingerprint = normalized_hash(sha256)
        with self.database.connect() as connection:
            cursor = connection.execute(
                """UPDATE model_files SET sha256=? WHERE id=? AND size_bytes=?
                AND modified_ns=? AND (sha256 IS NULL OR sha256<>?)""",
                (
                    fingerprint,
                    evidence.location_id,
                    evidence.size_bytes,
                    evidence.modified_ns,
                    fingerprint,
                ),
            )
            if cursor.rowcount:
                self.database.advance_revision(connection)
                return True
            row = connection.execute(
                "SELECT sha256,size_bytes,modified_ns FROM model_files WHERE id=?",
                (evidence.location_id,),
            ).fetchone()
            return bool(
                row is not None
                and row["sha256"] == fingerprint
                and row["size_bytes"] == evidence.size_bytes
                and row["modified_ns"] == evidence.modified_ns
            )

    def save_user_metadata(
        self,
        sha256: str,
        metadata: ModelMetadata,
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Publish a deliberate user choice independently of every provider match."""
        fingerprint = normalized_hash(sha256)
        payload = encode_metadata(metadata)
        decode_metadata(payload)
        owner = "user"
        with self.database.connect() as connection:
            existing = connection.execute(
                "SELECT payload FROM model_metadata WHERE sha256=? AND owner=?",
                (fingerprint, owner),
            ).fetchone()
            if existing is not None and existing["payload"] == payload:
                if prepared_assets_changed:
                    self.database.advance_revision(connection)
                return
            connection.execute(
                "INSERT OR REPLACE INTO model_metadata VALUES (?,?,?)",
                (fingerprint, owner, payload),
            )
            self.database.advance_revision(connection)

    def clear_provider_metadata(self, sha256: str) -> None:
        """Drop stale provider evidence without touching user metadata or thumbnail choices."""
        self.provider_cache.delete_for_hash(sha256)

    def save_provider_metadata(
        self,
        sha256: str,
        records: tuple[ProviderMetadataRecord, ...],
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Publish every exact provider match without merging their persistence ownership."""
        self.provider_cache.replace(
            sha256, records, prepared_assets_changed=prepared_assets_changed
        )
