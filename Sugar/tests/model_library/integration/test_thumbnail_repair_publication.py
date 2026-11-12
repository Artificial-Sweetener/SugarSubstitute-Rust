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
"""Prove automatic image repair publishes a new generation through shared HTTP consumers."""

from __future__ import annotations

import asyncio
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.thumbnail_freshness import (
    EnrichmentFreshnessFixture,
    fixture_provider_version,
)
from tests.support.no_progress import await_outcome


def test_background_repair_wakes_catalog_consumers_and_serves_the_same_asset(
    tmp_path: Path,
) -> None:
    """Rebuild missing bytes through the real scheduler without changing any model selection."""

    async def exercise() -> None:
        """Observe prepared files and change notifications through production runtime routes."""
        fixture = EnrichmentFreshnessFixture(tmp_path, fixture_provider_version())
        runtime = ModelLibraryRuntime(
            fixture.roots,
            enrichment_provider=fixture.provider,
            enrichment_previews=fixture.previews,
        )
        try:
            first = await runtime.automatic_enrichment.refresh(
                fixture.identity, automatic=True
            )
            await runtime.jobs.wait(first.job_id)
            entry = runtime.repository.list_entries()[0]
            assert entry.metadata is not None
            selected = entry.metadata.thumbnails[-1]
            asset_url = f"/sugarcubes/v1/models/assets/{selected.asset_id}"
            asset = runtime.assets.resolve(selected.asset_id)
            assert asset is not None
            asset.path.unlink()
            revision = runtime.repository.revision()
            routes = web.RouteTableDef()
            register_model_library_routes(routes, runtime)
            app = web.Application()
            app.add_routes(routes)
            async with TestClient(TestServer(app)) as client:
                unavailable = await client.get(asset_url)
                assert unavailable.status == 404
                await runtime.automatic_enrichment.refill()
                assert runtime.automatic_enrichment.status()["queued"] == 1
                runtime.automatic_enrichment.start()
                changed = await await_outcome(
                    client.get(
                        f"/sugarcubes/v1/models/changes?afterRevision={revision}"
                    )
                )
                payload: object = await changed.json()
                assert isinstance(payload, dict)
                assert (
                    isinstance(payload.get("revision"), int)
                    and payload["revision"] > revision
                )
                restored = await client.get(asset_url)
                assert (
                    restored.status == 200
                    and restored.headers["Content-Type"] == "image/webp"
                )
                current = runtime.repository.list_entries()[0]
                assert current.evidence.value == entry.evidence.value
                assert current.metadata == entry.metadata
                assert len(fixture.provider.hashes) == 2
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())
