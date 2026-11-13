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
"""Prove exact-value detector preferences and prepared assets through the real HTTP owners."""

from __future__ import annotations

import asyncio
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.detector_api import register_detector_routes
from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.model_roots import ObservedModelRoots
from tests.model_library.support.enrichment import ObservedProvider


class DetectorHttpRoots(ObservedModelRoots):
    """Keep detector preference and preview evidence in an isolated model folder."""

    def __init__(self, path: Path) -> None:
        """Bind only test storage rather than the workstation's configured model files."""
        super().__init__()
        self._path = path

    def storage_root(self) -> str:
        """Return the fixture's approved library root."""
        return str(self._path)


def test_detector_library_resolution_assignment_and_safety(tmp_path: Path) -> None:
    """Save one explicit art choice without selecting, hashing or acquiring a model."""

    async def exercise() -> None:
        """Use real routes and persistence with no remote provider or model generation."""
        roots = DetectorHttpRoots(tmp_path)
        runtime = ModelLibraryRuntime(roots, enrichment_provider=ObservedProvider())
        routes = web.RouteTableDef()
        register_model_library_routes(routes, runtime)
        register_detector_routes(routes, runtime.detectors, runtime.updates)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            library = await (
                await client.get("/sugarcubes/v1/models/detectors/library")
            ).json()
            assert len(library["models"]) == 17
            choice = library["models"][0]
            image = choice["thumbnails"][0]
            served = await client.get(image["url"])
            assert served.status == 200 and served.content_type == "image/webp"
            assert (await served.read())[:4] == b"RIFF"
            before = runtime.repository.revision()
            result = await client.put(
                "/sugarcubes/v1/models/detectors/association",
                json={"value": "literal<custom>.pt", "asset": "face-segmentation"},
            )
            assert result.status == 200 and runtime.repository.revision() > before
            resolved = await (
                await client.post(
                    "/sugarcubes/v1/models/detectors/resolve",
                    json={"values": ["literal<custom>.pt", "bbox/face_yolov8m.pt"]},
                )
            ).json()
            assert resolved["models"][0]["value"] == "literal<custom>.pt"
            assert resolved["models"][0]["detectorAsset"] == "face-segmentation"
            assert resolved["models"][1]["detectorAsset"] == "face-detection"
            for invalid in (
                {"values": "wrong"},
                {"values": [4]},
                {"values": [], "path": "private"},
            ):
                assert (
                    await client.post(
                        "/sugarcubes/v1/models/detectors/resolve", json=invalid
                    )
                ).status == 400
            assert (
                await client.put(
                    "/sugarcubes/v1/models/detectors/association",
                    json={"value": "literal<custom>.pt", "asset": "../../private"},
                )
            ).status == 400
            assert roots.scans == 0
        await runtime.jobs.close()
        await runtime.automatic_enrichment.close()

    asyncio.run(exercise())
