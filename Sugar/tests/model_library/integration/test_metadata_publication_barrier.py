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
"""Prove runtime maintenance waits for complete catalog and durable freshness publication."""

from __future__ import annotations

import asyncio
from pathlib import Path
from threading import Event

import pytest

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.enrichment import ObservedPreviews, ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.thumbnail_freshness import fixture_provider_version
from tests.support.no_progress import await_outcome, wait_for_event


@pytest.mark.parametrize("cancel", (False, True))
def test_maintenance_observes_terminal_publication_before_clearing_provider_state(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, cancel: bool
) -> None:
    """Retain the lease through catalog, freshness and cancellation bookkeeping writes."""
    (tmp_path / "fixture.safetensors").write_bytes(b"small identity fixture")
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path),
        enrichment_provider=ObservedProvider(fixture_provider_version(preview=False)),
        enrichment_previews=ObservedPreviews(),
    )
    runtime.catalog.refresh()
    initial = runtime.repository.list_entries()[0]
    started, release = Event(), Event()
    original = SqliteModelCatalogRepository.save_provider_metadata

    def save_provider_metadata(
        repository: SqliteModelCatalogRepository,
        sha256: str,
        records: tuple[ProviderMetadataRecord, ...],
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Hold the real SQLite publication boundary without replacing its persistence behavior."""
        started.set()
        wait_for_event(release, "Proof did not release catalog publication.")
        original(
            repository, sha256, records, prepared_assets_changed=prepared_assets_changed
        )

    monkeypatch.setattr(
        SqliteModelCatalogRepository, "save_provider_metadata", save_provider_metadata
    )

    async def exercise() -> None:
        """Queue exclusive maintenance during a real foreground enrichment job."""
        entered = asyncio.Event()
        observed: list[bool] = []

        async def maintenance() -> None:
            """Observe completed freshness, then clear provider labels after all prior writes finish."""
            async with runtime.metadata_activity.maintenance():
                entry = runtime.repository.find_entry(initial.evidence.location_id)
                assert entry is not None and entry.metadata is not None
                cache = SqliteModelEnrichmentCache(
                    runtime.repository.asset_root.parent / "enrichment-cache.sqlite3"
                )
                observed.append(cache.read(entry.evidence) is not None)
                runtime.repository.provider_cache.clear_metadata("civitai")
                entered.set()

        job = await runtime.refresh_metadata(initial.evidence.location_id)
        writer: asyncio.Task[None] | None = None
        try:
            await asyncio.to_thread(wait_for_event, started)
            writer = asyncio.create_task(maintenance())
            await asyncio.sleep(0)
            if cancel:
                runtime.jobs.cancel(job.job_id)
                await asyncio.sleep(0)
            assert not entered.is_set()
            release.set()
            await await_outcome(writer)
            result = await runtime.jobs.wait(job.job_id)
            assert result is not None and result.status == (
                "cancelled" if cancel else "completed"
            )
            assert observed == [not cancel]
            final = runtime.repository.find_entry(initial.evidence.location_id)
            assert final is not None and final.metadata is None
            assert final.evidence == initial.evidence
            assert runtime.repository.provider_cache.count("civitai") == 0
        finally:
            release.set()
            if writer is not None:
                await asyncio.gather(writer, return_exceptions=True)
            await runtime.jobs.close()

    asyncio.run(exercise())
