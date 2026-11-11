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
"""Prove automatic enrichment preserves previews and persists bounded retry decisions."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import asyncio
from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.model_library.application.automatic_enrichment import (
    AutomaticModelEnrichment,
)
from sugarcubes.model_library.application.enrichment_freshness import (
    ModelEnrichmentFreshness,
)
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.enrichment_service import (
    ModelEnrichmentService,
)
from sugarcubes.model_library.application.fingerprint_service import (
    ModelFingerprintService,
)
from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from sugarcubes.model_library.infrastructure.civitai_codec import parse_civitai_version
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.enrichment import (
    ObservedProvider,
    ObservedPreviews,
)


class FailingProvider(ObservedProvider):
    """Retain failures behind the same provider boundary as successful lookups."""

    async def lookup(self, sha256: str, kind: str) -> tuple[ProviderModelVersion, ...]:
        """Record the verified hash and simulate a transient provider outage."""
        self.hashes.append(sha256)
        raise ValueError("Temporary provider outage")


def test_missing_matches_survive_restart_and_changed_files_are_reconsidered(
    tmp_path: Path,
) -> None:
    """Retain confirmed provider misses until exact file evidence changes."""
    model = tmp_path / "fixture.safetensors"
    model.write_bytes(b"small fixture")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    catalog = ModelCatalogService(roots, repository, model_relationships())
    catalog.refresh()
    provider = ObservedProvider(None)
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        provider,
        ObservedPreviews(),
    )
    clock = [1000.0]
    state_path = repository.asset_root.parent / "enrichment.sqlite3"
    automatic = AutomaticModelEnrichment(
        repository,
        service,
        SqliteEnrichmentAttempts(state_path),
        ModelEnrichmentFreshness(
            SqliteModelEnrichmentCache(
                repository.asset_root.parent / "enrichment-cache.sqlite3"
            ),
            service.previews,
            lambda _kind: "fixture-policy",
            lambda _kind: "fixture-provider",
            lambda _kind: ("civitai",),
        ),
        lambda: clock[0],
    )
    identity = repository.list_entries()[0].evidence.location_id
    assert asyncio.run(automatic.refresh(identity))["status"] == "not_found"
    restarted = AutomaticModelEnrichment(
        repository,
        service,
        SqliteEnrichmentAttempts(state_path),
        ModelEnrichmentFreshness(
            SqliteModelEnrichmentCache(
                repository.asset_root.parent / "enrichment-cache.sqlite3"
            ),
            service.previews,
            lambda _kind: "fixture-policy",
            lambda _kind: "fixture-provider",
            lambda _kind: ("civitai",),
        ),
        lambda: clock[0],
    )
    assert asyncio.run(restarted.refresh(identity))["status"] == "cached"
    assert len(provider.hashes) == 1
    model.write_bytes(b"changed fixture bytes")
    catalog.refresh()
    assert asyncio.run(restarted.refresh(identity))["status"] == "not_found"
    assert len(provider.hashes) == 2
    clock[0] += 86401
    assert asyncio.run(restarted.refresh(identity))["status"] == "cached"
    assert len(provider.hashes) == 2


def test_automatic_metadata_keeps_existing_preview_and_manual_refresh_remains_available(
    tmp_path: Path,
) -> None:
    """Keep automatic provider labels below deliberate local images and allow explicit refresh."""
    (tmp_path / "fixture.safetensors").write_bytes(b"fixture")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    entry = repository.list_entries()[0]
    old = (ThumbnailReference("b" * 64, "standard", 128, 128, "image/webp"),)
    repository.save_local_metadata(
        entry.evidence, "fixture", ModelMetadata("Local", thumbnails=old)
    )
    version = parse_civitai_version(
        {
            "id": 20,
            "modelId": 10,
            "name": "v2",
            "model": {"name": "Provider"},
            "images": [{"url": "https://image.civitai.com/safe.jpg", "nsfwLevel": 1}],
        }
    )
    provider = ObservedProvider(version)
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        provider,
        ObservedPreviews(),
    )
    automatic = AutomaticModelEnrichment(
        repository,
        service,
        SqliteEnrichmentAttempts(repository.asset_root.parent / "enrichment.sqlite3"),
        ModelEnrichmentFreshness(
            SqliteModelEnrichmentCache(
                repository.asset_root.parent / "enrichment-cache.sqlite3"
            ),
            service.previews,
            lambda _kind: "fixture-policy",
            lambda _kind: "fixture-provider",
            lambda _kind: ("civitai",),
        ),
        lambda: 1000,
    )
    assert (
        asyncio.run(automatic.refresh(entry.evidence.location_id))["status"] == "found"
    )
    current = repository.list_entries()[0]
    assert current.metadata is not None and current.metadata.thumbnails == old
    assert (
        asyncio.run(automatic.refresh(entry.evidence.location_id))["status"] == "cached"
    )
    assert len(provider.hashes) == 1
    assert (
        asyncio.run(automatic.refresh(entry.evidence.location_id, automatic=False))[
            "status"
        ]
        == "found"
    )
    assert len(provider.hashes) == 2
    current = repository.list_entries()[0]
    assert (
        current.metadata is not None
        and current.metadata.thumbnails[0].asset_id == "a" * 64
    )


def test_provider_failure_cooldown_is_shared_without_extending_duplicate_deadlines(
    tmp_path: Path,
) -> None:
    """Reuse content decisions across locations and retry transient failures after five minutes."""
    for name in ("one", "two"):
        (tmp_path / f"{name}.safetensors").write_bytes(b"identical fixture")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    provider = FailingProvider()
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        provider,
        ObservedPreviews(),
    )
    clock = [1000.0]
    path = repository.asset_root.parent / "enrichment.sqlite3"
    automatic = AutomaticModelEnrichment(
        repository,
        service,
        SqliteEnrichmentAttempts(path),
        ModelEnrichmentFreshness(
            SqliteModelEnrichmentCache(
                repository.asset_root.parent / "enrichment-cache.sqlite3"
            ),
            service.previews,
            lambda _kind: "fixture-policy",
            lambda _kind: "fixture-provider",
            lambda _kind: ("civitai",),
        ),
        lambda: clock[0],
    )
    first, second = repository.list_entries()
    with pytest.raises(ValueError, match="Temporary provider outage"):
        asyncio.run(automatic.refresh(first.evidence.location_id))
    assert (
        asyncio.run(automatic.refresh(second.evidence.location_id))["status"]
        == "cached"
    )
    restarted = AutomaticModelEnrichment(
        repository,
        service,
        SqliteEnrichmentAttempts(path),
        ModelEnrichmentFreshness(
            SqliteModelEnrichmentCache(
                repository.asset_root.parent / "enrichment-cache.sqlite3"
            ),
            service.previews,
            lambda _kind: "fixture-policy",
            lambda _kind: "fixture-provider",
            lambda _kind: ("civitai",),
        ),
        lambda: clock[0],
    )
    assert (
        asyncio.run(restarted.refresh(first.evidence.location_id))["status"] == "cached"
    )
    assert len(provider.hashes) == 1
    clock[0] += 301
    with pytest.raises(ValueError, match="Temporary provider outage"):
        asyncio.run(restarted.refresh(second.evidence.location_id))
    assert len(provider.hashes) == 2


def test_changed_file_lease_cannot_be_replaced_by_a_late_old_result(
    tmp_path: Path,
) -> None:
    """Keep stale completion and interrupted leases from suppressing new file evidence."""
    (tmp_path / "fixture.safetensors").write_bytes(b"fixture")
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(
        FixtureFileRoots(tmp_path), repository, model_relationships()
    ).refresh()
    old = repository.list_entries()[0].evidence
    changed = replace(
        old, size_bytes=old.size_bytes + 1, modified_ns=old.modified_ns + 1
    )
    attempts = SqliteEnrichmentAttempts(
        repository.asset_root.parent / "enrichment.sqlite3"
    )
    assert attempts.reserve(old, 1000, policy="fixture-policy", providers=("civitai",))
    assert not attempts.reserve(
        old, 1001, policy="fixture-policy", providers=("civitai",)
    )
    assert attempts.reserve(
        changed, 1001, policy="fixture-policy", providers=("civitai",)
    )
    attempts.finish(
        old, None, "not_found", 100000, policy="fixture-policy", providers=("civitai",)
    )
    assert not attempts.reserve(
        changed, 1002, policy="fixture-policy", providers=("civitai",)
    )
    assert attempts.reserve(
        changed, 1902, policy="fixture-policy", providers=("civitai",)
    )
