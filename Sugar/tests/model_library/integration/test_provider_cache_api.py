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
"""Prove HTTP and host cache actions share one runtime and job lifecycle."""

from __future__ import annotations

import asyncio
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library import host_api
from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.cache_api import register_model_cache_routes
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.model_roots import FixtureFileRoots


def test_host_and_http_share_validated_actions_and_observable_committed_usage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reject arbitrary actions and expose the same completed cache job to both consumers."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    routes = web.RouteTableDef()
    register_model_library_routes(routes, runtime)
    register_model_cache_routes(routes, runtime)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Observe real local HTTP without external provider calls or model hashing."""
        async with TestClient(TestServer(app)) as client:
            response = await client.get("/sugarcubes/v1/models/cache/civitai")
            assert (
                response.status == 200
                and response.headers["Cache-Control"] == "no-store"
            )
            initial = await response.json()
            assert initial == await host_api.prepared_civitai_cache_summary()
            assert initial["providerRecordCount"] == initial["thumbnailBytes"] == 0
            for action in ("delete", "clear_all", "openmodeldb"):
                response = await client.post(
                    f"/sugarcubes/v1/models/cache/civitai/{action}"
                )
                assert response.status == 400
            with pytest.raises(ValueError):
                host_api.queue_civitai_cache_action({"action": "clear_metadata"})
            response = await client.post(
                "/sugarcubes/v1/models/cache/civitai/clear_thumbnails"
            )
            assert response.status == 202
            payload = await response.json()
            job = await runtime.jobs.wait(payload["jobId"])
            assert job is not None and job.status == "completed"
            assert job.result == initial
            assert await runtime.updates.wait(0) > 0
            assert await (
                await client.get(f"/sugarcubes/v1/models/jobs/{job.job_id}")
            ).json() == host_api.model_job(job.job_id)
            host_job = host_api.queue_civitai_cache_action("clear_metadata")
            result = await runtime.jobs.wait(str(host_job["jobId"]))
            assert result is not None and result.status == "completed"
            assert result.result == await host_api.prepared_civitai_cache_summary()
        await runtime.jobs.close()

    asyncio.run(exercise())
