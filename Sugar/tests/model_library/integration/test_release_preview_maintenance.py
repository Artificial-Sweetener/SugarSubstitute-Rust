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
"""Prove cached version reads retain the runtime cache lease through physical work."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path
from threading import Event

import pytest

from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from tests.model_library.support.model_releases import release
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.thumbnail_freshness import PreparedFixturePreviews
from tests.support.no_progress import wait_for_event


class HeldPreviewRead(PreparedFixturePreviews):
    """Hold a real prepared-file snapshot until its asynchronous owner can drain it."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Expose explicit worker entry and release without timing-dependent delays."""
        super().__init__(assets)
        self.entered = Event()
        self.proceed = Event()

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Capture real availability before holding the blocking adapter operation."""
        result = super().available(thumbnails)
        self.entered.set()
        wait_for_event(
            self.proceed, "Preview proof did not release the availability worker."
        )
        return result


def fixture_runtime(
    root: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[ModelLibraryRuntime, HeldPreviewRead, str]:
    """Compose shared runtime owners around tiny files and a deterministic remote family."""
    (root / "fixture.safetensors").write_bytes(b"tiny preview fixture")
    images = HeldPreviewRead(
        ModelThumbnailAssets(root / ".sugarcubes" / "model-library" / "v1" / "assets")
    )
    runtime = ModelLibraryRuntime(FixtureFileRoots(root), enrichment_previews=images)
    images.assets = runtime.assets
    runtime.catalog.refresh()
    entry = runtime.repository.list_entries()[0]
    runtime.repository.save_fingerprint(entry.evidence, "b" * 64)
    runtime.repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    "Fixture",
                    base_model="SDXL 1.0",
                    provider="civitai",
                    provider_model_id="10",
                    provider_version_id="20",
                )
            ),
        ),
    )

    async def family(
        _provider: CivitaiModelProvider, model_id: str
    ) -> tuple[ModelProviderRelease, ...]:
        """Replace only remote history while retaining actual family eligibility."""
        assert model_id == "10"
        current = release("20")
        return (
            replace(
                current,
                version=replace(
                    current.version,
                    preview_url="https://image.civitai.com/fixture.jpg",
                ),
            ),
        )

    monkeypatch.setattr(CivitaiModelProvider, "releases", family)
    return runtime, images, entry.evidence.location_id


@pytest.mark.parametrize("close_previews", (False, True))
def test_cached_preview_read_drains_before_maintenance(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, close_previews: bool
) -> None:
    """Prevent cache collection or host teardown from overtaking a cached availability worker."""
    runtime, images, identity = fixture_runtime(tmp_path, monkeypatch)

    async def exercise() -> None:
        """Control reader, teardown and writer entry through explicit synchronization signals."""
        writer_started, writer_entered = asyncio.Event(), asyncio.Event()
        cached_read: asyncio.Task[tuple[ThumbnailReference, ...]] | None = None
        writer: asyncio.Task[None] | None = None
        shutdown: asyncio.Task[None] | None = None

        async def maintenance() -> None:
            """Collect actual provider-owned bytes under the runtime's exclusive owner."""
            writer_started.set()
            async with runtime.metadata_activity.maintenance():
                writer_entered.set()
                runtime.assets.clear_sources("civitai")

        try:
            original = await runtime.release_previews.get(identity, "20")
            assert original and len(images.urls) == 1
            cached_read = asyncio.create_task(
                runtime.release_previews.get(identity, "20")
            )
            await asyncio.to_thread(wait_for_event, images.entered)
            if close_previews:
                shutdown = asyncio.create_task(runtime.release_previews.close())
                await asyncio.sleep(0)
            writer = asyncio.create_task(maintenance())
            await writer_started.wait()
            assert not writer_entered.is_set()
            if shutdown is not None:
                assert not shutdown.done()
            images.proceed.set()
            if close_previews:
                with pytest.raises(asyncio.CancelledError):
                    await cached_read
                assert shutdown is not None
                await shutdown
            else:
                assert await cached_read == original
            await writer
            assert writer_entered.is_set()
            assert not images.available(original)
            if not close_previews:
                assert await runtime.release_previews.get(identity, "20") == original
                assert images.available(original)
                assert len(images.urls) == 2
        finally:
            images.proceed.set()
            await runtime.release_previews.close()
            await runtime.jobs.close()
            for task in (cached_read, writer, shutdown):
                if task is not None:
                    await asyncio.gather(task, return_exceptions=True)

    asyncio.run(exercise())
