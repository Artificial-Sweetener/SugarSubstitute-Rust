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
"""Prove cache maintenance drains prior work and survives cancellation or failure."""

from __future__ import annotations

import asyncio
from pathlib import Path
from threading import Event

import pytest

from sugarcubes.model_library.application.provider_registry import (
    ModelMetadataProviders,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.civitai_transport import (
    CivitaiTransport,
    CivitaiResourceMissing,
)
from sugarcubes.model_library.infrastructure.provider_cache_store import (
    CivitaiCacheStorage,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.enrichment import ObservedProvider, ObservedPreviews
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.thumbnail_freshness import fixture_provider_version
from tests.support.no_progress import wait_for_event


@pytest.mark.parametrize("cancel", (False, True))
def test_maintenance_waits_for_prior_publication_and_can_cancel_before_commit(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, cancel: bool
) -> None:
    """Never clear evidence beneath an active writer or publish a cancelled maintenance action."""
    (tmp_path / "fixture.safetensors").write_bytes(b"tiny maintenance fixture")
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path),
        enrichment_provider=ObservedProvider(fixture_provider_version(preview=False)),
        enrichment_previews=ObservedPreviews(),
    )
    runtime.catalog.refresh()
    entry = runtime.repository.list_entries()[0]
    started, release = Event(), Event()
    original = SqliteModelCatalogRepository.save_provider_metadata

    def publish(
        repository: SqliteModelCatalogRepository,
        sha256: str,
        records: tuple[ProviderMetadataRecord, ...],
        *,
        prepared_assets_changed: bool = False,
    ) -> None:
        """Hold the real catalog writer so maintenance must retain its exclusive wait."""
        started.set()
        wait_for_event(release, "Proof did not release prior publication.")
        original(
            repository, sha256, records, prepared_assets_changed=prepared_assets_changed
        )

    monkeypatch.setattr(SqliteModelCatalogRepository, "save_provider_metadata", publish)

    async def exercise() -> None:
        """Cancel only before irreversible maintenance begins, then prove a later action still succeeds."""
        prior = await runtime.refresh_metadata(entry.evidence.location_id)
        try:
            await asyncio.to_thread(wait_for_event, started)
            clear = runtime.cache_actions.start("clear_metadata")
            await asyncio.sleep(0)
            assert clear.cancellable and clear.status == "running"
            if cancel:
                runtime.jobs.cancel(clear.job_id)
                await runtime.jobs.wait(clear.job_id)
            assert not prior.status == "completed"
            release.set()
            await runtime.jobs.wait(prior.job_id)
            await runtime.jobs.wait(clear.job_id)
            assert clear.status == ("cancelled" if cancel else "completed")
            assert runtime.repository.provider_cache.count("civitai") == int(cancel)
            later = runtime.cache_actions.start("clear_metadata")
            await runtime.jobs.wait(later.job_id)
            assert later.status == "completed"
            assert runtime.repository.provider_cache.count("civitai") == 0
        finally:
            release.set()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_committed_maintenance_finishes_despite_late_cancellation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep a committed multi-store transition non-cancellable until publication completes."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    started, release = Event(), Event()
    original = CivitaiCacheStorage.clear_metadata

    def clear(storage: CivitaiCacheStorage) -> None:
        """Pause inside the physical maintenance writer while retaining its real behavior."""
        started.set()
        wait_for_event(release, "Proof did not release cache maintenance.")
        original(storage)

    monkeypatch.setattr(CivitaiCacheStorage, "clear_metadata", clear)

    async def exercise() -> None:
        """Observe the shared job instead of relying on an HTTP request's lifetime."""
        job = runtime.cache_actions.start("clear_metadata")
        try:
            await asyncio.to_thread(wait_for_event, started)
            assert not job.cancellable and job.status == "running"
            runtime.jobs.cancel(job.job_id)
            assert job.status == "running"
            release.set()
            await runtime.jobs.wait(job.job_id)
            assert job.status == "completed"
        finally:
            release.set()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_failed_maintenance_releases_queue_and_retains_actionable_diagnostics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    """Make failure observable and permit later maintenance instead of stranding the scheduler."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))

    def fail(storage: CivitaiCacheStorage) -> None:
        """Model an external storage failure at the owning persistence boundary."""
        raise OSError("Fixture cache volume unavailable")

    monkeypatch.setattr(CivitaiCacheStorage, "clear_thumbnails", fail)

    async def exercise() -> None:
        """Read a fresh summary and finish a later action after the first job fails."""
        job = runtime.cache_actions.start("clear_thumbnails")
        await runtime.jobs.wait(job.job_id)
        assert job.status == "failed"
        assert (await runtime.civitai_cache.summary()).thumbnail_bytes == 0
        later = runtime.cache_actions.start("clear_metadata")
        await runtime.jobs.wait(later.job_id)
        assert later.status == "completed"
        await runtime.jobs.close()

    asyncio.run(exercise())
    assert "Fixture cache volume unavailable" in caplog.text


@pytest.mark.parametrize(
    "action,requests", (("clear_thumbnails", 1), ("clear_metadata", 2), ("refresh", 2))
)
def test_cache_actions_reset_process_misses_only_when_new_lookup_is_intended(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, action: str, requests: int
) -> None:
    """Keep thumbnail collection separate from the CivitAI transport's confirmed hash misses."""
    calls: list[str] = []

    async def missing(transport: CivitaiTransport, url: str) -> object:
        """Replace only the external provider transport with a confirmed missing response."""
        calls.append(url)
        raise CivitaiResourceMissing("Fixture missing content")

    monkeypatch.setattr(CivitaiTransport, "read_json", missing)
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    registry = runtime.enrichment.provider
    assert isinstance(registry, ModelMetadataProviders)
    provider = registry.registrations[1].provider
    assert isinstance(provider, CivitaiModelProvider)

    async def exercise() -> None:
        """Exercise the actual shared provider before and after the maintenance job."""
        assert await provider.lookup("a" * 64) is None
        assert await provider.lookup("a" * 64) is None
        assert len(calls) == 1
        job = runtime.cache_actions.start(action)
        await runtime.jobs.wait(job.job_id)
        assert job.status == "completed"
        assert await provider.lookup("a" * 64) is None
        assert len(calls) == requests
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
