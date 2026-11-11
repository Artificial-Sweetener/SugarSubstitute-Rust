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
"""Prove desktop identity queries share verified storage without provider calls."""

from __future__ import annotations

from tests.model_library.support.enrichment import ObservedProvider

import asyncio
import hashlib
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.fingerprint_api import register_model_fingerprint_routes
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.model_roots import FixtureFileRoots


def test_explicit_fingerprint_job_and_cached_lookup(tmp_path: Path) -> None:
    """Fingerprint a tiny approved file, reuse it, and invalidate it after a stat change."""
    content = b"identity fixture, never model weights"
    model = tmp_path / "fixture.safetensors"
    model.write_bytes(content)

    async def exercise() -> None:
        """Exercise the HTTP boundary with isolated files and no remote requests."""
        runtime = ModelLibraryRuntime(
            FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
        )
        runtime.catalog.refresh()
        entry = runtime.repository.list_entries()[0]
        digest = hashlib.sha256(content).hexdigest()
        routes = web.RouteTableDef()
        register_model_fingerprint_routes(routes, runtime)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get(
                f"/sugarcubes/v1/models/by-hash/{digest}?kind=loras"
            )
            payload: object = await response.json()
            assert isinstance(payload, dict)
            assert payload["models"] == [] and payload["unfingerprintedCount"] == 1
            response = await client.post(
                f"/sugarcubes/v1/models/{entry.evidence.location_id}/fingerprint/refresh"
            )
            assert response.status == 202
            payload = await response.json()
            assert isinstance(payload, dict) and isinstance(payload["jobId"], str)
            job = await runtime.jobs.wait(payload["jobId"])
            assert job is not None and job.status == "completed"
            assert job.result["sha256"] == digest
            response = await client.get(
                f"/sugarcubes/v1/models/by-hash/{digest.upper()}?kind=loras"
            )
            payload = await response.json()
            assert isinstance(payload, dict)
            assert payload["models"][0]["value"] == model.name
            assert payload["unfingerprintedCount"] == 0
            model.write_bytes(b"changed identity")
            runtime.catalog.refresh()
            response = await client.get(
                f"/sugarcubes/v1/models/by-hash/{digest}?kind=loras"
            )
            payload = await response.json()
            assert isinstance(payload, dict)
            assert payload["models"] == [] and payload["unfingerprintedCount"] == 1
            assert (
                await client.get("/sugarcubes/v1/models/by-hash/invalid?kind=loras")
            ).status == 400
            assert (
                await client.get(f"/sugarcubes/v1/models/by-hash/{digest}")
            ).status == 400
            assert (
                await client.post("/sugarcubes/v1/models/missing/fingerprint/refresh")
            ).status == 404
        await runtime.automatic_enrichment.close()
        await runtime.jobs.close()

    asyncio.run(exercise())
