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
"""Prove catalog and prepared images cross the real HTTP boundary safely."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from tests.model_library.support.enrichment import ObservedProvider

import asyncio
from io import BytesIO
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.release_api import register_model_release_routes
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.model_roots import ObservedModelRoots


class HttpModelRoots(ObservedModelRoots):
    """Select isolated model-library storage for the HTTP integration fixture."""

    def __init__(self, root: Path) -> None:
        """Keep the fixture's files and port isolated from any running Comfy host."""
        super().__init__()
        self._root = root

    def storage_root(self) -> str:
        """Expose the temporary storage directory to the composition boundary."""
        return str(self._root)


def test_http_catalog_assets_and_job_errors(tmp_path: Path) -> None:
    """Serve literal metadata and cached images while keeping raw file paths private."""

    async def exercise() -> None:
        """Run a real isolated aiohttp service without models or provider traffic."""
        roots = HttpModelRoots(tmp_path)
        runtime = ModelLibraryRuntime(roots, enrichment_provider=ObservedProvider())
        runtime.catalog.refresh()
        runtime.repository.save_fingerprint(roots.evidence, "a" * 64)
        stream = BytesIO()
        Image.new("RGB", (100, 150), (32, 60, 90)).save(stream, "PNG")
        thumbnails = ModelThumbnailPreparer(runtime.assets, origin="user").prepare(
            stream.getvalue()
        )
        runtime.repository.save_provider_metadata(
            "a" * 64,
            (
                ProviderMetadataRecord(
                    ModelMetadata("<script>literal</script>", thumbnails=thumbnails)
                ),
            ),
        )
        routes = web.RouteTableDef()
        register_model_library_routes(routes, runtime)
        register_model_release_routes(routes, runtime)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get("/sugarcubes/v1/models?kind=loras")
            assert response.status == 200
            payload: object = await response.json()
            assert isinstance(payload, dict)
            assert payload["models"][0]["value"] == roots.evidence.value
            assert payload["models"][0]["title"] == "<script>literal</script>"
            assert str(tmp_path) not in await response.text()
            asset = await client.get(
                f"/sugarcubes/v1/models/assets/{thumbnails[0].asset_id}"
            )
            assert asset.status == 200
            assert asset.headers["Content-Type"] == "image/webp"
            assert "immutable" in asset.headers["Cache-Control"]
            content = await asset.read()
            with Image.open(BytesIO(content)) as decoded:
                assert decoded.size == (thumbnails[0].width, thumbnails[0].height)
            missing = await client.get("/sugarcubes/v1/models/jobs/missing")
            assert missing.status == 404
            invalid = await client.get("/sugarcubes/v1/models/assets/invalid")
            assert invalid.status == 400
            capabilities = await client.get("/sugarcubes/v1/models/capabilities")
            assert capabilities.status == 200
            invalid_revision = await client.get(
                "/sugarcubes/v1/models/changes?afterRevision=-1"
            )
            assert invalid_revision.status == 400
            changes = await client.get("/sugarcubes/v1/models/changes?afterRevision=0")
            assert changes.status == 200
            changed_payload: object = await changes.json()
            assert isinstance(changed_payload, dict) and changed_payload["revision"] > 0
            invalid_model = await client.post(
                "/sugarcubes/v1/models/invalid/metadata/refresh"
            )
            assert invalid_model.status == 404
            saved = await client.put(
                f"/sugarcubes/v1/models/{roots.evidence.location_id}/thumbnail",
                data=stream.getvalue(),
            )
            assert saved.status == 200
            rejected = await client.put(
                f"/sugarcubes/v1/models/{roots.evidence.location_id}/thumbnail",
                data=b"not an image",
            )
            assert rejected.status == 400
            assert runtime.repository.list_entries()[0].metadata is not None
            history = await client.get(
                f"/sugarcubes/v1/models/{roots.evidence.location_id}/versions"
            )
            assert history.status == 400
            assert "Refresh metadata" in await history.text()
            unsafe = await client.post(
                "/sugarcubes/v1/models/missing/acquire",
                json={
                    "versionId": "20",
                    "fileId": "30",
                    "downloadUrl": "https://127.0.0.1/private",
                },
            )
            assert unsafe.status == 400
            queued = await client.post(
                "/sugarcubes/v1/models/missing/acquire",
                json={"versionId": "20", "fileId": "30"},
            )
            assert queued.status == 202
            queued_payload: object = await queued.json()
            assert isinstance(queued_payload, dict) and isinstance(
                queued_payload.get("jobId"), str
            )
            job = await runtime.jobs.wait(queued_payload["jobId"])
            assert job is not None and job.status == "failed"
            assert not tuple(tmp_path.glob("*.safetensors"))
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
