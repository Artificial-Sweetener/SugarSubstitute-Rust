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
"""Safeguard reference cache freshness for policy, files, misses and prepared assets."""

from __future__ import annotations

import asyncio
from contextlib import closing
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
import sqlite3

import pytest
from sugarcubes.model_library.domain.models import ModelEvidence, ModelMetadata
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)

from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)


@pytest.mark.parametrize("preview,found", [(False, False), (False, True)])
def test_completed_provider_decisions_remain_fresh_after_restart_and_a_day(
    tmp_path: Path, preview: bool, found: bool
) -> None:
    """Retain exact confirmed misses and matches without thumbnails until file evidence changes."""
    fixture = EnrichmentFreshnessFixture(
        tmp_path, fixture_provider_version(preview=preview) if found else None
    )
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == (
        "found" if found else "not_found"
    )
    fixture.clock[0] += 86401
    restarted = fixture.restart()
    assert asyncio.run(restarted.refresh(fixture.identity))["status"] == "cached"
    assert len(fixture.provider.hashes) == 1
    fixture.model.write_bytes(b"changed tiny fixture")
    fixture.catalog.refresh()
    assert asyncio.run(restarted.refresh(fixture.identity))["status"] == (
        "found" if found else "not_found"
    )
    assert len(fixture.provider.hashes) == 2


def test_missing_prepared_variant_is_rebuilt_without_waiting_a_day(
    tmp_path: Path,
) -> None:
    """Reconsider selected images when their stored prepared files disappear."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None
    missing = fixture.previews.assets.resolve(current.metadata.thumbnails[-1].asset_id)
    assert missing is not None
    missing.path.unlink()
    revision = fixture.repository.revision()
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert missing.path.is_file()
    assert fixture.repository.revision() > revision
    assert len(fixture.provider.hashes) == 2 and len(fixture.previews.urls) == 2
    assert (
        asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "cached"
    )


def test_changed_thumbnail_policy_reselects_provider_images_immediately(
    tmp_path: Path,
) -> None:
    """Invalidate successful image decisions by policy identity instead of an arbitrary retry deadline."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    fixture.policy[0] = "civitai-thumbnail-v1:allow_soft"
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert len(fixture.provider.hashes) == 2 and len(fixture.previews.urls) == 2


def test_animation_policy_reconsiders_cached_matches_without_preview(
    tmp_path: Path,
) -> None:
    """Refresh earlier excluded-media decisions when first-frame preparation becomes available."""
    fixture = EnrichmentFreshnessFixture(
        tmp_path, fixture_provider_version(preview=False)
    )
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    assert not fixture.previews.urls
    fixture.provider.version = fixture_provider_version()
    fixture.policy[0] = "civitai-thumbnail-v2:sfw_only"
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert len(fixture.previews.urls) == 1


def test_preview_failure_retains_backoff_then_recovers_missing_assets(
    tmp_path: Path,
) -> None:
    """Keep a thumbnail failure retryable even after successful metadata lookup."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    fixture.previews.unavailable = True
    first = asyncio.run(fixture.automatic.refresh(fixture.identity))
    assert first["status"] == "found" and "warning" in first
    assert (
        asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "cached"
    )
    fixture.clock[0] += 301
    fixture.previews.unavailable = False
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert len(fixture.provider.hashes) == 2
    assert (
        asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "cached"
    )


def test_local_previews_keep_precedence_across_policy_changes(tmp_path: Path) -> None:
    """Keep valid local images above provider selection after provider labels have been cached."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    local = asyncio.run(fixture.previews.prepare("local-fixture"))
    entry = fixture.repository.list_entries()[0]
    fixture.repository.save_local_metadata(
        entry.evidence, "local-fixture", ModelMetadata("Local", thumbnails=local)
    )
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    fixture.policy[0] = "civitai-thumbnail-v1:allow_soft"
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert fixture.previews.urls == ["local-fixture"]
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.thumbnails == local
    assert current.thumbnail_origin == "local"


def test_user_choice_preserves_image_after_provider_labels_and_policy_changes(
    tmp_path: Path,
) -> None:
    """Recognize the current image owner even when an earlier provider decision had no local image."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    entry = fixture.repository.list_entries()[0]
    chosen = asyncio.run(fixture.previews.prepare("user-choice"))
    fixture.repository.save_thumbnail_choice(entry.evidence, chosen)
    fixture.policy[0] = "civitai-thumbnail-v1:allow_soft"
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.thumbnails == chosen
    assert current.thumbnail_origin == "user"
    assert len(fixture.previews.urls) == 2


def test_new_provider_miss_removes_provider_state_and_keeps_user_choice(
    tmp_path: Path,
) -> None:
    """Drop a disproved provider link and image while preserving deliberate user presentation."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    entry = fixture.repository.list_entries()[0]
    assert entry.sha256 is not None
    chosen = asyncio.run(fixture.previews.prepare("user-choice"))
    fixture.repository.save_user_metadata(entry.sha256, ModelMetadata("User title"))
    assert fixture.repository.save_thumbnail_choice(entry.evidence, chosen)
    fixture.provider.version = None
    assert (
        asyncio.run(fixture.automatic.refresh(fixture.identity, automatic=False))[
            "status"
        ]
        == "not_found"
    )
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.title == "User title"
    assert current.metadata.links() == () and current.metadata.thumbnails == chosen
    assert (
        asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "cached"
    )


def test_disabled_thumbnail_policy_clears_old_provider_image(tmp_path: Path) -> None:
    """Remove incompatible automatic provider images when the active policy selects no image."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    fixture.provider.version = fixture_provider_version(preview=False)
    fixture.policy[0] = "civitai-thumbnail-v1:disabled"
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    current = fixture.repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.thumbnails == ()
    assert (
        asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "cached"
    )


_EVIDENCE_CHANGES: tuple[Callable[[ModelEvidence], ModelEvidence], ...] = (
    lambda evidence: replace(evidence, value="renamed"),
    lambda evidence: replace(evidence, kind="vae"),
    lambda evidence: replace(evidence, root_id="another"),
    lambda evidence: replace(evidence, relative_path="another.pth"),
    lambda evidence: replace(evidence, size_bytes=1),
    lambda evidence: replace(evidence, modified_ns=1),
)


@pytest.mark.parametrize(
    "change",
    _EVIDENCE_CHANGES,
    ids=("value", "kind", "root", "path", "size", "modified"),
)
def test_freshness_checks_every_host_file_evidence_field(
    tmp_path: Path, change: Callable[[ModelEvidence], ModelEvidence]
) -> None:
    """Invalidate a completed record whenever host selection or stat evidence changes."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    entry = fixture.repository.list_entries()[0]
    cache = SqliteModelEnrichmentCache(
        fixture.repository.asset_root.parent / "enrichment-cache.sqlite3"
    )
    assert cache.read(entry.evidence) is not None
    assert cache.read(change(entry.evidence)) is None


def test_corrupt_freshness_evidence_is_replaced_with_observable_recovery(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """Preserve diagnostics and recover a cache row instead of trusting invalid persisted identities."""
    fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
    assert asyncio.run(fixture.automatic.refresh(fixture.identity))["status"] == "found"
    path = fixture.repository.asset_root.parent / "enrichment-cache.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute("UPDATE enrichment_records SET sha256='invalid'")
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert "Stored model enrichment evidence is invalid" in caplog.text
    assert len(fixture.provider.hashes) == 2


def test_future_cache_schema_is_preserved(tmp_path: Path) -> None:
    """Reject unsupported persistence versions without mutating their contents."""
    path = tmp_path / "future.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute("PRAGMA user_version=2")
        connection.execute("CREATE TABLE future (value TEXT)")
        connection.execute("INSERT INTO future VALUES ('preserve')")
    before = path.read_bytes()
    with pytest.raises(ValueError, match="preserved"):
        SqliteModelEnrichmentCache(path)
    assert path.read_bytes() == before


def test_legacy_retry_store_upgrade_preserves_deadlines_without_applying_unknown_policy(
    tmp_path: Path,
) -> None:
    """Keep original retry records while assigning new operations an explicit policy identity."""
    path = tmp_path / "attempts.sqlite3"
    with closing(sqlite3.connect(path)) as connection, connection:
        connection.execute(
            "CREATE TABLE content_attempts (sha256 TEXT PRIMARY KEY,status TEXT NOT NULL,retry_at REAL NOT NULL)"
        )
        connection.execute(
            "INSERT INTO content_attempts VALUES (?,?,?)", ("a" * 64, "failed", 1300)
        )
    attempts = SqliteEnrichmentAttempts(path)
    assert attempts.content_retry_at("a" * 64, policy="") == 1300
    assert attempts.content_retry_at("a" * 64, policy="current") == 0


def test_distinct_thumbnail_policies_keep_independent_failure_deadlines(
    tmp_path: Path,
) -> None:
    """Prevent late completion under an old policy from replacing current policy retry evidence."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    evidence = fixture.repository.list_entries()[0].evidence
    attempts = SqliteEnrichmentAttempts(tmp_path / "attempts.sqlite3")
    for policy, deadline in (("soft", 1600), ("safe", 1300)):
        assert attempts.reserve(evidence, 1000, policy=policy, providers=("civitai",))
        attempts.finish(
            evidence,
            "a" * 64,
            "failed",
            deadline,
            policy=policy,
            providers=("civitai",),
        )
    assert attempts.content_retry_at("a" * 64, policy="soft") == 1600
    assert attempts.content_retry_at("a" * 64, policy="safe") == 1300


def test_confirmed_miss_is_reconsidered_when_another_provider_becomes_eligible(
    tmp_path: Path,
) -> None:
    """Keep partial-provider misses from suppressing lookup after provider preferences change."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    fixture.provider_scope[0] = "openmodeldb"
    assert (
        asyncio.run(fixture.automatic.refresh(fixture.identity))["status"]
        == "not_found"
    )
    fixture.provider_scope[0] = "openmodeldb,civitai"
    fixture.provider.version = fixture_provider_version()
    assert asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "found"
    assert len(fixture.provider.hashes) == 2


def test_thumbnail_only_policy_change_retains_a_confirmed_miss(tmp_path: Path) -> None:
    """Apply reference negative-cache freshness independently of image selection preferences."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    assert (
        asyncio.run(fixture.automatic.refresh(fixture.identity))["status"]
        == "not_found"
    )
    fixture.policy[0] = "civitai-thumbnail-v1:disabled"
    assert (
        asyncio.run(fixture.restart().refresh(fixture.identity))["status"] == "cached"
    )
    assert len(fixture.provider.hashes) == 1
