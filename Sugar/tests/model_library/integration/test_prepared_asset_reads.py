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
"""Prove shared prepared-image metadata remains cheap and stable during cache collection."""

from __future__ import annotations

from io import BytesIO
import hashlib
import os
from pathlib import Path

from PIL import Image
import pytest

from sugarcubes.model_library import host_api
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_assets import PreparedAsset
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.model_roots import FixtureFileRoots


def test_host_metadata_does_not_restat_an_asset_after_concurrent_collection(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Return the asset owner's observed metadata even if collection follows resolution.

    The public path is a snapshot for a sibling server's normal file response;
    resolving it does not pin the file indefinitely. A later lookup must report
    absence rather than turning a legitimate cache clear into an exception.
    """
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    content = BytesIO()
    Image.new("RGB", (12, 18), "blue").save(content, "PNG")
    images = ModelThumbnailPreparer(runtime.assets, origin="civitai").prepare(
        content.getvalue()
    )
    resolve = runtime.assets.resolve
    selected = resolve(images[0].asset_id)
    assert selected is not None
    evidence = selected.path.stat()

    def resolve_then_collect(identity: str) -> PreparedAsset | None:
        """Interleave real cache collection after the file owner validates its result."""
        asset = resolve(identity)
        runtime.assets.clear_sources("civitai")
        return asset

    monkeypatch.setattr(runtime.assets, "resolve", resolve_then_collect)
    payload = host_api.prepared_model_asset(selected.asset_id)
    assert payload == {
        "assetId": selected.asset_id,
        "path": str(selected.path),
        "contentType": "image/webp",
        "modifiedNs": str(evidence.st_mtime_ns),
        "sizeBytes": evidence.st_size,
    }
    assert not selected.path.exists()
    assert host_api.prepared_model_asset(selected.asset_id) is None


def test_host_prepared_asset_reads_avoid_redundant_stats_and_image_processing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep rapid sibling-server access independent of preview generation and provider work."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    content = BytesIO()
    Image.new("RGB", (12, 18), "blue").save(content, "WEBP")
    identity = runtime.assets.publish(content.getvalue())
    asset = runtime.assets.resolve(identity)
    assert asset is not None
    stat = Path.stat
    calls = 0

    def observed_stat(path: Path, *, follow_symlinks: bool = True) -> os.stat_result:
        """Count only the selected image's filesystem metadata boundary."""
        nonlocal calls
        if path == asset.path:
            calls += 1
        return stat(path, follow_symlinks=follow_symlinks)

    monkeypatch.setattr(Path, "stat", observed_stat)

    def forbid_image_processing(*args: object, **kwargs: object) -> None:
        """Fail if a prepared-file consumer rehashes bytes or decodes a source image."""
        raise AssertionError("Prepared image lookup must not process image bytes")

    monkeypatch.setattr(Image, "open", forbid_image_processing)
    monkeypatch.setattr(hashlib, "sha256", forbid_image_processing)
    for _ in range(3):
        payload = host_api.prepared_model_asset(identity)
        assert payload is not None and payload["sizeBytes"] == len(content.getvalue())
    # Windows path canonicalization also stats the selected file to reject namespace escapes.
    assert calls <= 6
