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
"""Safeguard legacy catalog images before provider cache collection."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from contextlib import closing
import asyncio
from io import BytesIO
from pathlib import Path
import sqlite3

from PIL import Image
import pytest

from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.thumbnail_freshness import EnrichmentFreshnessFixture


def test_runtime_registers_all_persisted_thumbnail_owners_before_collection(
    tmp_path: Path,
) -> None:
    """Keep hidden local and user choices when they share old provider bytes."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    repository = fixture.repository
    entry = repository.list_entries()[0]
    images = _legacy_images(fixture)
    local = _colored_images(fixture, "red")
    chosen = _colored_images(fixture, "purple")
    provider_only = _colored_images(fixture, "green")
    repository.save_fingerprint(entry.evidence, "b" * 64)
    repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata("Provider", provider="civitai", thumbnails=images)
            ),
        ),
    )
    repository.save_user_metadata(
        "b" * 64, ModelMetadata("Chosen", provider="civitai", thumbnails=images)
    )
    repository.save_local_metadata(
        entry.evidence, "local-signature", ModelMetadata("Local", thumbnails=local)
    )
    repository.save_thumbnail_choice(entry.evidence, chosen)
    repository.save_provider_metadata(
        "c" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    "Only provider", provider="civitai", thumbnails=provider_only
                )
            ),
        ),
    )
    with closing(
        sqlite3.connect(repository.asset_root / "sources.sqlite3")
    ) as connection:
        with connection:
            connection.execute("DELETE FROM thumbnail_source_variants")
            connection.execute("DELETE FROM thumbnail_sources")
    before = repository.snapshot()
    runtime = ModelLibraryRuntime(fixture.roots)
    assert runtime.assets.source_summary("civitai").source_count == 2
    assert runtime.assets.source_summary("user").source_count == 2
    assert runtime.assets.source_summary("local").source_count == 1
    runtime.assets.clear_sources("civitai")
    assert all(
        runtime.assets.resolve(item.asset_id) is not None
        for item in (*images, *local, *chosen)
    )
    assert all(runtime.assets.resolve(item.asset_id) is None for item in provider_only)
    assert repository.snapshot() == before
    reopened = ModelLibraryRuntime(fixture.roots)
    assert reopened.assets.source_summary("user").source_count == 2
    assert reopened.assets.source_summary("local").source_count == 1
    assert repository.snapshot() == before


def _legacy_images(
    fixture: EnrichmentFreshnessFixture,
) -> tuple[ThumbnailReference, ...]:
    """Prepare the fixture's real source without relying on remote image transport."""
    return asyncio.run(
        fixture.previews.prepare("https://image.civitai.com/fixture.jpg")
    )


def test_backfill_does_not_duplicate_registered_source_usage(tmp_path: Path) -> None:
    """Keep repeated runtime construction from manufacturing extra source records."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    images = _legacy_images(fixture)
    entry = fixture.repository.list_entries()[0]
    fixture.repository.save_fingerprint(entry.evidence, "b" * 64)
    fixture.repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata("Provider", provider="civitai", thumbnails=images)
            ),
        ),
    )
    before = fixture.previews.assets.source_summary("civitai")
    runtime = ModelLibraryRuntime(fixture.roots)
    assert runtime.assets.source_summary("civitai") == before
    ModelLibraryRuntime(fixture.roots)
    assert runtime.assets.source_summary("civitai") == before


@pytest.mark.parametrize("provider", ("huggingface", "openmodeldb"))
def test_backfill_preserves_missing_and_other_provider_legacy_references(
    tmp_path: Path,
    provider: str,
) -> None:
    """Retain known ownership without guessing a provider or requiring intact bytes."""
    fixture = EnrichmentFreshnessFixture(tmp_path, None)
    images = _legacy_images(fixture)
    entry = fixture.repository.list_entries()[0]
    fixture.repository.save_fingerprint(entry.evidence, "b" * 64)
    fixture.repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata("Unknown origin", provider=provider, thumbnails=images)
            ),
        ),
    )
    asset = fixture.previews.assets.resolve(images[-1].asset_id)
    assert asset is not None
    asset.path.unlink()
    runtime = ModelLibraryRuntime(fixture.roots)
    runtime.assets.clear_sources("civitai")
    assert all(
        runtime.assets.resolve(item.asset_id) is not None
        for item in images
        if item.asset_id != images[-1].asset_id
    )
    assert fixture.repository.list_entries()[0].metadata == ModelMetadata(
        "Unknown origin", provider=provider, thumbnails=images
    )


def _colored_images(
    fixture: EnrichmentFreshnessFixture, color: str
) -> tuple[ThumbnailReference, ...]:
    """Supply distinct legacy image sources to prove each hidden owner's retention."""
    stream = BytesIO()
    Image.new("RGB", (96, 144), color).save(stream, "PNG")
    return ModelThumbnailPreparer(fixture.previews.assets, origin="civitai").prepare(
        stream.getvalue()
    )
