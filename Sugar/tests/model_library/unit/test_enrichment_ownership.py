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
"""Prove durable image and lookup ownership survives restart and additive migration."""

from __future__ import annotations

from contextlib import closing
from pathlib import Path
import sqlite3

import pytest

from sugarcubes.model_library.domain.enrichment_freshness import ModelEnrichmentRecord
from sugarcubes.model_library.domain.models import (
    ModelEvidence,
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.domain.thumbnail_sources import ThumbnailSourceOrigin
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
    enrichment_evidence_signature,
)
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from sugarcubes.model_library.infrastructure.metadata_codec import encode_metadata

EVIDENCE = ModelEvidence(
    "upscale_models", "selected.pth", "primary", "selected.pth", 12, 100
)
IMAGE = (ThumbnailReference("b" * 64, "standard", 128, 192, "image/webp"),)


@pytest.mark.parametrize("origin", ("civitai", "openmodeldb", "user", "local"))
def test_explicit_borrowed_image_origin_survives_restart_and_provider_collection(
    tmp_path: Path, origin: ThumbnailSourceOrigin
) -> None:
    """Address images by source ownership instead of the provider supplying their labels."""
    path = tmp_path / "cache.sqlite3"
    metadata = ModelMetadata(
        "OpenModelDB title", provider="openmodeldb", thumbnails=IMAGE
    )
    record = ModelEnrichmentRecord(
        "a" * 64, "policy", metadata, "both", False, origin, ("openmodeldb", "civitai")
    )
    SqliteModelEnrichmentCache(path).save(EVIDENCE, record)
    reopened = SqliteModelEnrichmentCache(path)
    assert reopened.read(EVIDENCE) == record
    reopened.clear_thumbnails("civitai")
    current = SqliteModelEnrichmentCache(path).read(EVIDENCE)
    assert current is not None and current.metadata is not None
    assert current.metadata.title == "OpenModelDB title"
    assert current.providers == record.providers
    assert current.metadata.thumbnails == (() if origin == "civitai" else IMAGE)
    assert current.thumbnail_origin == (None if origin == "civitai" else origin)


@pytest.mark.parametrize(
    "local_preview,expected", ((False, "civitai"), (True, "local"))
)
def test_legacy_decision_migrates_known_scope_and_preserves_local_preview_ownership(
    tmp_path: Path, local_preview: bool, expected: str
) -> None:
    """Keep historical local fallback images independent of cached CivitAI labels."""
    path = tmp_path / "legacy.sqlite3"
    metadata = ModelMetadata("CivitAI title", provider="civitai", thumbnails=IMAGE)
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute(
            "CREATE TABLE enrichment_records(id TEXT PRIMARY KEY,signature TEXT,sha256 TEXT,policy TEXT,metadata TEXT,local_preview INTEGER,provider_scope TEXT)"
        )
        connection.execute(
            "INSERT INTO enrichment_records VALUES (?,?,?,?,?,?,?)",
            (
                EVIDENCE.location_id,
                enrichment_evidence_signature(EVIDENCE),
                "a" * 64,
                "policy",
                encode_metadata(metadata),
                int(local_preview),
                "openmodeldb,civitai",
            ),
        )
        connection.execute("PRAGMA user_version=1")
    cache = SqliteModelEnrichmentCache(path)
    record = cache.read(EVIDENCE)
    assert record is not None and record.thumbnail_origin == expected
    assert record.providers == ("openmodeldb", "civitai")
    cache.clear_thumbnails("civitai")
    current = SqliteModelEnrichmentCache(path).read(EVIDENCE)
    assert current is not None and current.metadata is not None
    assert current.metadata.thumbnails == (IMAGE if local_preview else ())


def test_legacy_retry_migration_resets_civitai_leases_and_preserves_openmodeldb_cooldowns(
    tmp_path: Path,
) -> None:
    """Retain historical production provider ownership without parsing unknown scopes as CivitAI."""
    path = tmp_path / "attempts.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute(
            "CREATE TABLE file_attempts(id TEXT PRIMARY KEY,signature TEXT,status TEXT,retry_at REAL)"
        )
        connection.execute(
            "CREATE TABLE content_attempts(sha256 TEXT,status TEXT,retry_at REAL,policy TEXT,PRIMARY KEY(sha256,policy))"
        )
        connection.execute(
            "INSERT INTO file_attempts VALUES (?,?,?,?)",
            (
                EVIDENCE.location_id,
                f"civitai|policy:{enrichment_evidence_signature(EVIDENCE)}",
                "running",
                2000,
            ),
        )
        for scope in ("civitai", "openmodeldb", "openmodeldb,civitai", "unknown"):
            connection.execute(
                "INSERT INTO content_attempts VALUES (?,?,?,?)",
                ("a" * 64, "failed", 2000, f"{scope}|policy"),
            )
    attempts = SqliteEnrichmentAttempts(path)
    attempts.clear_provider("civitai")
    assert attempts.content_retry_at("a" * 64, policy="civitai|policy") == 0
    assert attempts.content_retry_at("a" * 64, policy="openmodeldb,civitai|policy") == 0
    assert attempts.content_retry_at("a" * 64, policy="openmodeldb|policy") == 2000
    assert attempts.content_retry_at("a" * 64, policy="unknown|policy") == 2000
    assert attempts.reserve(
        EVIDENCE, 1000, policy="civitai|policy", providers=("civitai",)
    )


def test_corrupt_image_ownership_is_diagnosed_without_mutating_the_stored_record(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """Reconsider invalid evidence without rewriting it into a guessed provider origin."""
    path = tmp_path / "cache.sqlite3"
    cache = SqliteModelEnrichmentCache(path)
    cache.save(
        EVIDENCE,
        ModelEnrichmentRecord(
            "a" * 64,
            "policy",
            ModelMetadata("Image", thumbnails=IMAGE),
            "civitai",
            thumbnail_origin="civitai",
            providers=("civitai",),
        ),
    )
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute("UPDATE enrichment_records SET thumbnail_origin='invalid'")
    assert SqliteModelEnrichmentCache(path).read(EVIDENCE) is None
    assert "Stored model enrichment evidence is invalid" in caplog.text
    with closing(sqlite3.connect(path)) as connection:
        assert (
            connection.execute(
                "SELECT thumbnail_origin FROM enrichment_records"
            ).fetchone()[0]
            == "invalid"
        )
