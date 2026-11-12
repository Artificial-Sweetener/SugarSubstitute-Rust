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
"""Prove provider cache actions through the real runtime and durable stores."""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.thumbnail_freshness import (
    PreparedFixturePreviews,
    fixture_provider_version,
)
from tests.support.no_progress import await_outcome


@pytest.mark.parametrize("found", (False, True))
@pytest.mark.parametrize("action", ("clear_thumbnails", "clear_metadata", "refresh"))
def test_cache_actions_preserve_file_evidence_and_reference_decision_states(
    tmp_path: Path, found: bool, action: str
) -> None:
    """Keep clearing distinct from refreshing, including durable confirmed misses."""
    (tmp_path / "fixture.safetensors").write_bytes(b"small cache fixture")
    provider = ObservedProvider(fixture_provider_version() if found else None)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=provider
    )
    previews = PreparedFixturePreviews(runtime.assets)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path),
        enrichment_provider=provider,
        enrichment_previews=previews,
    )
    runtime.catalog.refresh()
    initial = runtime.repository.list_entries()[0]
    cache = SqliteModelEnrichmentCache(
        runtime.repository.asset_root.parent / "enrichment-cache.sqlite3"
    )

    async def exercise() -> None:
        """Observe the maintenance job without starting scans or model execution."""
        enriched = await runtime.refresh_metadata(initial.evidence.location_id)
        await runtime.jobs.wait(enriched.job_id)
        assert enriched.status == "completed"
        before = await runtime.civitai_cache.summary()
        assert before.provider_record_count == int(found)
        revision = runtime.repository.revision()
        job = runtime.cache_actions.start(action)
        await runtime.jobs.wait(job.job_id)
        assert job.status == "completed", job.error
        assert runtime.repository.revision() > revision
        entry = runtime.repository.list_entries()[0]
        assert entry.evidence == initial.evidence
        assert entry.sha256 is not None
        record = cache.read(entry.evidence)
        after = await runtime.civitai_cache.summary()
        if action == "clear_metadata":
            assert record is None and entry.metadata is None
            assert after.provider_record_count == 0
        else:
            assert record is not None
            assert after.provider_record_count == int(found)
        if action == "clear_thumbnails":
            assert record is not None
            assert record.metadata is None or record.metadata.thumbnails == ()
            assert after.thumbnail_source_count == 0
            assert after.thumbnail_variant_count == 0
            assert after.thumbnail_bytes == 0
            assert entry.metadata is None or entry.metadata.thumbnails == ()
        assert len(provider.hashes) == 1
        if action == "refresh":

            async def refreshed() -> None:
                """Observe the requested refresh even though the prior durable decision is still fresh."""
                while len(provider.hashes) < 2:
                    await asyncio.sleep(0)
                active = runtime.jobs.active(
                    f"metadata-refresh:{entry.evidence.location_id}"
                )
                if active is not None:
                    await runtime.jobs.wait(active.job_id)

            await await_outcome(refreshed())
            assert len(provider.hashes) == 2
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
