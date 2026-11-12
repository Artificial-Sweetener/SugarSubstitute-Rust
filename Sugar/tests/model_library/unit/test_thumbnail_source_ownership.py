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
"""Safeguard provider cache collection against shared user, local and bundled assets."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
from pathlib import Path
import sqlite3
from contextlib import closing
from dataclasses import replace
from threading import Event

from PIL import Image
import pytest

from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.domain.thumbnail_sources import (
    ThumbnailOwnership,
    ThumbnailSourceOrigin,
)
from tests.support.no_progress import future_result, wait_for_event


def test_retention_rolls_back_the_whole_batch_on_invalid_legacy_references(
    tmp_path: Path,
) -> None:
    """Keep partial backfill from authorizing collection with incomplete ownership."""
    assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(assets, origin="civitai").prepare(
        source_bytes("red")
    )
    invalid = replace(images[0], asset_id="../invalid")
    with pytest.raises(ValueError, match="SHA256"):
        assets.retain_sources(
            (
                ThumbnailOwnership("user", images),
                ThumbnailOwnership("local", (invalid,)),
            )
        )
    assert assets.source_summary("user").source_count == 0
    assert assets.source_summary("local").source_count == 0
    assert all(assets.resolve(item.asset_id) is not None for item in images)


def test_retention_accepts_existing_image_contract_without_decoding(
    tmp_path: Path,
) -> None:
    """Protect persisted image declarations independently from the prepared output format."""
    assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(assets, origin="civitai").prepare(
        source_bytes("red")
    )
    declared_png = tuple(replace(item, content_type="image/png") for item in images)
    assets.retain_sources((ThumbnailOwnership("user", declared_png),))
    assets.clear_sources("civitai")
    assert all(assets.resolve(item.asset_id) is not None for item in images)


def source_bytes(color: str) -> bytes:
    """Create a small real portrait without model weights or provider requests."""
    stream = BytesIO()
    Image.new("RGB", (180, 270), color).save(stream, "PNG")
    return stream.getvalue()


def test_provider_clear_preserves_shared_user_and_other_provider_assets(
    tmp_path: Path,
) -> None:
    """Release only the chosen provider's ownership of physically shared prepared bytes."""
    assets = ModelThumbnailAssets(tmp_path)
    civitai = ModelThumbnailPreparer(assets, origin="civitai")
    user = ModelThumbnailPreparer(assets, origin="user")
    other = ModelThumbnailPreparer(assets, origin="openmodeldb")
    shared = civitai.prepare(source_bytes("red"))
    assert user.prepare(source_bytes("red")) == shared
    only_provider = civitai.prepare(source_bytes("green"))
    other_images = other.prepare(source_bytes("blue"))
    summary = assets.source_summary("civitai")
    assert summary.source_count == 2
    assert summary.variant_count == len(
        {item.asset_id for item in (*shared, *only_provider)}
    )
    assert summary.byte_size > 0
    assets.clear_sources("civitai")
    assert assets.source_summary("civitai").source_count == 0
    assert all(
        assets.resolve(item.asset_id) is not None for item in (*shared, *other_images)
    )
    assert all(assets.resolve(item.asset_id) is None for item in only_provider)
    assert assets.source_summary("user").source_count == 1
    assert assets.source_summary("openmodeldb").source_count == 1


@pytest.mark.parametrize("origin", ("local", "bundled"))
def test_provider_collection_preserves_other_presentation_origins(
    tmp_path: Path, origin: ThumbnailSourceOrigin
) -> None:
    """Keep source ownership explicit for local and curated illustrations."""
    assets = ModelThumbnailAssets(tmp_path)
    image = source_bytes("purple")
    provider_images = ModelThumbnailPreparer(assets, origin="civitai").prepare(image)
    assets.prepare_source(origin, "a" * 64, lambda: provider_images)
    reopened = ModelThumbnailAssets(tmp_path)
    reopened.clear_sources("civitai")
    assert all(reopened.resolve(item.asset_id) is not None for item in provider_images)
    assert reopened.source_summary(origin).source_count == 1


def test_repeated_and_concurrent_preparation_has_one_durable_source(
    tmp_path: Path,
) -> None:
    """Serialize publication with ownership across independent library instances."""
    content = source_bytes("navy")
    first = ModelThumbnailPreparer(ModelThumbnailAssets(tmp_path), origin="civitai")
    second = ModelThumbnailPreparer(ModelThumbnailAssets(tmp_path), origin="civitai")
    with ThreadPoolExecutor(max_workers=2) as pool:
        results = tuple(
            pool.map(lambda preparer: preparer.prepare(content), (first, second))
        )
    assert results[0] == results[1]
    reopened = ModelThumbnailAssets(tmp_path)
    summary = reopened.source_summary("civitai")
    assert summary.source_count == 1
    assert summary.variant_count == len({item.asset_id for item in results[0]})
    assert summary.byte_size == sum(
        path.stat().st_size for path in tmp_path.glob("*.webp")
    )


def test_failed_source_publication_keeps_previous_ownership(tmp_path: Path) -> None:
    """Leave a committed source available when a later preparation fails."""
    assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(assets, origin="civitai").prepare(
        source_bytes("red")
    )
    before = assets.source_summary("civitai")

    def failing() -> tuple[ThumbnailReference, ...]:
        """Model an image codec failure before source publication."""
        raise ValueError("Fixture image decode failed.")

    with pytest.raises(ValueError, match="decode failed"):
        assets.prepare_source("civitai", "b" * 64, failing)
    assert assets.source_summary("civitai") == before
    assert all(assets.resolve(item.asset_id) is not None for item in images)


def test_cache_usage_counts_only_existing_files_and_collection_preserves_unknown_assets(
    tmp_path: Path,
) -> None:
    """Do not guess ownership from filenames or count externally removed prepared bytes."""
    assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(assets, origin="civitai").prepare(
        source_bytes("gold")
    )
    unknown = ModelThumbnailPreparer(assets, origin="user").prepare(
        source_bytes("silver")
    )
    with (
        closing(sqlite3.connect(tmp_path / "sources.sqlite3")) as connection,
        connection,
    ):
        connection.execute("DELETE FROM thumbnail_sources WHERE origin='user'")
    first = assets.resolve(images[0].asset_id)
    assert first is not None
    first.path.unlink()
    assert (
        assets.source_summary("civitai").variant_count
        == len({item.asset_id for item in images}) - 1
    )
    assets.clear_sources("civitai")
    assert all(assets.resolve(item.asset_id) is not None for item in unknown)


def test_unknown_source_schema_is_preserved(tmp_path: Path) -> None:
    """Reject future ownership schemas before collecting any asset."""
    with (
        closing(sqlite3.connect(tmp_path / "sources.sqlite3")) as connection,
        connection,
    ):
        connection.execute("PRAGMA user_version=99")
    original = (tmp_path / "sources.sqlite3").read_bytes()
    with pytest.raises(ValueError, match="schema"):
        ModelThumbnailAssets(tmp_path)
    assert (tmp_path / "sources.sqlite3").read_bytes() == original


def test_collection_waits_for_another_origins_publication(tmp_path: Path) -> None:
    """Hold the source lease across publication so a concurrent clear retains the new owner."""
    provider_assets = ModelThumbnailAssets(tmp_path)
    other_assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(provider_assets, origin="civitai").prepare(
        source_bytes("red")
    )
    publishing = Event()
    finish_publication = Event()
    clearing = Event()

    def publish_user() -> tuple[ThumbnailReference, ...]:
        """Block a deterministic publication while it holds its database write lease."""

        def generate() -> tuple[ThumbnailReference, ...]:
            """Reuse tiny prepared files after the test allows ownership publication."""
            publishing.set()
            wait_for_event(finish_publication, "Fixture publication was not released.")
            return images

        return other_assets.prepare_source("user", "c" * 64, generate)

    def clear_provider() -> None:
        """Exercise collection through an independent library instance."""
        clearing.set()
        provider_assets.clear_sources("civitai")

    with ThreadPoolExecutor(max_workers=2) as pool:
        publication = pool.submit(publish_user)
        wait_for_event(publishing)
        collection = pool.submit(clear_provider)
        wait_for_event(clearing)
        finish_publication.set()
        assert future_result(publication) == images
        future_result(collection)
    assert provider_assets.source_summary("civitai").source_count == 0
    assert other_assets.source_summary("user").source_count == 1
    assert all(other_assets.resolve(item.asset_id) is not None for item in images)


def test_valid_cached_source_avoids_decoding_and_missing_variant_is_rebuilt(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reuse complete prepared variants, then repair an externally removed file under the same identity."""
    assets = ModelThumbnailAssets(tmp_path)
    preparer = ModelThumbnailPreparer(assets, origin="civitai")
    content = source_bytes("orange")
    images = preparer.prepare(content)

    def unavailable_codec(*_args: object, **_kwargs: object) -> Image.Image:
        """Fail only if the pipeline unnecessarily decodes an already prepared source."""
        raise OSError("Fixture codec unavailable.")

    monkeypatch.setattr(Image, "open", unavailable_codec)
    assert preparer.prepare(content) == images
    first = assets.resolve(images[0].asset_id)
    assert first is not None
    first.path.unlink()
    with pytest.raises(OSError, match="codec unavailable"):
        preparer.prepare(content)
    monkeypatch.undo()
    assert preparer.prepare(content) == images
    assert all(assets.resolve(item.asset_id) is not None for item in images)


def test_invalid_origin_and_asset_identity_fail_before_collection(
    tmp_path: Path,
) -> None:
    """Reject untrusted source namespaces and reference paths without touching existing bytes."""
    assets = ModelThumbnailAssets(tmp_path)
    images = ModelThumbnailPreparer(assets, origin="user").prepare(source_bytes("red"))
    with pytest.raises(ValueError, match="origin"):
        assets.clear_sources("../user")
    with pytest.raises(ValueError, match="SHA256"):
        assets.prepare_source("civitai", "../../private", lambda: images)
    invalid = (ThumbnailReference("../../private", "standard", 32, 32, "image/webp"),)
    with pytest.raises(ValueError, match="SHA256"):
        assets.prepare_source("civitai", "d" * 64, lambda: invalid)
    assert assets.source_summary("user").source_count == 1
    assert all(assets.resolve(item.asset_id) is not None for item in images)
