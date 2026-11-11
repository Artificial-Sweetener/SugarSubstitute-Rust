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
"""Prove startup publishes shared images and validates visible-model priority."""

from __future__ import annotations

import asyncio
from io import BytesIO
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.infrastructure.civitai_codec import parse_civitai_version
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots


class PreparedFixturePreview:
    """Replace remote image transport while exercising production thumbnail preparation."""

    def __init__(self, root: Path) -> None:
        """Build tiny images in the production shared asset namespace."""
        stream = BytesIO()
        Image.new("RGB", (24, 32), "green").save(stream, format="PNG")
        self.content = stream.getvalue()
        self.preparer = ModelThumbnailPreparer(
            ModelThumbnailAssets(root / ".sugarcubes/model-library/v1/assets"),
            origin="civitai",
        )
        self.prepared = asyncio.Event()

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Publish browser and desktop variants without external traffic."""
        result = self.preparer.prepare(self.content)
        self.prepared.set()
        return result

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Delegate fixture asset checks to the production prepared-image owner."""
        return self.preparer.available(thumbnails)


def test_startup_prepares_thumbnails_without_an_explicit_refresh(
    tmp_path: Path,
) -> None:
    """Observe automatic enrichment through catalog and immutable image routes."""
    (tmp_path / "fixture.safetensors").write_bytes(b"fixture-only")

    async def exercise() -> None:
        """Compose production scheduling with isolated external boundaries."""
        previews = PreparedFixturePreview(tmp_path)
        provider = ObservedProvider(
            parse_civitai_version(
                {
                    "id": 20,
                    "modelId": 10,
                    "name": "v2",
                    "model": {"name": "Fixture Model"},
                    "images": [
                        {"url": "https://image.civitai.com/fixture.jpg", "nsfwLevel": 1}
                    ],
                }
            )
        )
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path),
            enrichment_provider=provider,
            enrichment_previews=previews,
        )
        initial = runtime.start_initial_scan()
        await runtime.jobs.wait(initial.job_id)
        await asyncio.wait_for(previews.prepared.wait(), timeout=3)
        identity = runtime.repository.list_entries()[0].evidence.location_id
        active = runtime.jobs.active(f"metadata-refresh:{identity}")
        if active is not None:
            await runtime.jobs.wait(active.job_id)
        assert len(provider.hashes) == 1
        entries = runtime.repository.list_entries()
        assert (
            entries[0].metadata is not None and len(entries[0].metadata.thumbnails) == 4
        )
        routes = web.RouteTableDef()
        register_model_library_routes(routes, runtime)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get("/sugarcubes/v1/models?kind=loras")
            payload: object = await response.json()
            assert (
                isinstance(payload, dict)
                and payload["models"][0]["title"] == "Fixture Model"
            )
            asset = await client.get(payload["models"][0]["thumbnails"][0]["url"])
            assert asset.status == 200 and asset.headers["Content-Type"] == "image/webp"
            for invalid in [
                {"modelIds": ["../outside"]},
                {"modelIds": ["a" * 64] * 129},
                None,
            ]:
                rejected = await client.post(
                    "/sugarcubes/v1/models/metadata/prioritize", json=invalid
                )
                assert rejected.status == 400
            missing = await client.post(
                "/sugarcubes/v1/models/metadata/prioritize",
                json={"modelIds": ["a" * 64]},
            )
            assert missing.status == 404
            accepted = await client.post(
                "/sugarcubes/v1/models/metadata/prioritize",
                json={"modelIds": [entries[0].evidence.location_id]},
            )
            assert accepted.status == 202
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
