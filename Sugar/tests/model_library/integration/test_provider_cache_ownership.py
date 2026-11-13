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
"""Safeguard independent providers, protected shared images and durable retry ownership."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from io import BytesIO
from pathlib import Path

from PIL import Image
import pytest

from sugarcubes.model_library.domain.enrichment_freshness import ModelEnrichmentRecord
from sugarcubes.model_library.domain.models import ModelEvidence, ModelMetadata
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.provider_links import ModelProviderLink
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.model_roots import FixtureFileRoots

LINKS = (
    ModelProviderLink(
        "openmodeldb",
        "OpenModelDB",
        "4x-Fixture",
        "4x-Fixture",
        "https://openmodeldb.info/models/4x-Fixture",
    ),
    ModelProviderLink(
        "civitai", "CivitAI", "10", "20", "https://civitai.com/models/10"
    ),
)


@pytest.mark.parametrize("action", ("clear_thumbnails", "clear_metadata"))
def test_civitai_maintenance_preserves_other_provider_and_user_shared_bytes(
    tmp_path: Path, action: str
) -> None:
    """Clear borrowed CivitAI previews without deleting the same bytes deliberately chosen by a user."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    evidence = ModelEvidence(
        "upscale_models", "chosen.pth", "primary", "chosen.pth", 12, 100
    )
    other = replace(evidence, value="other.pth", relative_path="other.pth")
    runtime.repository.reconcile((evidence, other), None)
    runtime.repository.save_fingerprint(evidence, "a" * 64)
    runtime.repository.save_fingerprint(other, "b" * 64)
    stream = BytesIO()
    Image.new("RGB", (96, 144), "navy").save(stream, "PNG")
    thumbnails = ModelThumbnailPreparer(runtime.assets, origin="civitai").prepare(
        stream.getvalue()
    )
    protected = ModelThumbnailPreparer(runtime.assets, origin="user").prepare(
        stream.getvalue()
    )
    assert thumbnails == protected
    openmodeldb = ModelMetadata(
        "OpenModelDB title",
        provider="openmodeldb",
        provider_url=LINKS[0].page_url,
        provider_model_id=LINKS[0].model_id,
        provider_version_id=LINKS[0].version_id,
        thumbnails=thumbnails,
    )
    civitai = ModelMetadata(
        "CivitAI title",
        provider="civitai",
        provider_url=LINKS[1].page_url,
        provider_model_id="10",
        provider_version_id="20",
    )
    runtime.repository.save_provider_metadata(
        "a" * 64,
        (
            ProviderMetadataRecord(openmodeldb, "civitai"),
            ProviderMetadataRecord(civitai),
        ),
    )
    runtime.repository.save_thumbnail_choice(evidence, protected)
    cache = SqliteModelEnrichmentCache(
        runtime.repository.asset_root.parent / "enrichment-cache.sqlite3"
    )
    decision = ModelEnrichmentRecord(
        "a" * 64,
        "policy",
        openmodeldb,
        "both",
        False,
        "civitai",
        ("openmodeldb", "civitai"),
    )
    cache.save(evidence, decision)
    other_decision = ModelEnrichmentRecord(
        "b" * 64, "policy", None, "openmodeldb", providers=("openmodeldb",)
    )
    cache.save(other, other_decision)
    attempts = SqliteEnrichmentAttempts(
        runtime.repository.asset_root.parent / "enrichment.sqlite3"
    )
    assert attempts.reserve(
        evidence, 1000, policy="joint-policy", providers=("openmodeldb", "civitai")
    )
    attempts.finish(
        evidence,
        "a" * 64,
        "failed",
        2000,
        policy="joint-policy",
        providers=("openmodeldb", "civitai"),
    )
    assert attempts.reserve(
        other, 1000, policy="other-policy", providers=("openmodeldb",)
    )
    attempts.finish(
        other,
        "b" * 64,
        "failed",
        3000,
        policy="other-policy",
        providers=("openmodeldb",),
    )

    async def exercise() -> None:
        """Verify the exact operation result and reopen all durable evidence afterward."""
        before = await runtime.civitai_cache.summary()
        assert before.provider_record_count == before.thumbnail_source_count == 1
        job = runtime.cache_actions.start(action)
        await runtime.jobs.wait(job.job_id)
        assert job.status == "completed", job.error
        assert not job.cancellable
        current = runtime.repository.find_entry(evidence.location_id)
        assert current is not None and current.metadata is not None
        assert current.metadata.title == "OpenModelDB title"
        assert (
            current.metadata.thumbnails == protected
            and current.thumbnail_origin == "user"
        )
        assert runtime.repository.provider_cache.metadata(
            "a" * 64, "openmodeldb"
        ) == replace(openmodeldb, thumbnails=())
        assert all(
            runtime.assets.resolve(item.asset_id) is not None for item in protected
        )
        assert runtime.assets.source_summary("civitai").source_count == 0
        assert runtime.assets.source_summary("user").source_count == 1
        reopened = SqliteModelEnrichmentCache(
            runtime.repository.asset_root.parent / "enrichment-cache.sqlite3"
        )
        assert reopened.read(other) == other_decision
        if action == "clear_metadata":
            assert reopened.read(evidence) is None
            assert attempts.content_retry_at("a" * 64, policy="joint-policy") == 0
            assert attempts.reserve(
                evidence, 1001, policy="joint-policy", providers=("civitai",)
            )
            assert current.metadata.links() == (LINKS[0],)
        else:
            record = reopened.read(evidence)
            assert record == replace(
                decision,
                metadata=replace(openmodeldb, thumbnails=()),
                thumbnail_origin=None,
            )
            assert attempts.content_retry_at("a" * 64, policy="joint-policy") == 2000
        assert attempts.content_retry_at("b" * 64, policy="other-policy") == 3000
        assert not attempts.reserve(
            other, 1001, policy="other-policy", providers=("openmodeldb",)
        )
        await runtime.jobs.close()

    asyncio.run(exercise())
