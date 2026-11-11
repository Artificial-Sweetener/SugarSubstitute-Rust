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
"""Prove initial pulls, daily selections and deliberate version browsing share checked history."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from datetime import timedelta
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library.application.model_update_feed import ModelUpdateFeed
from sugarcubes.model_library.infrastructure.release_history_store import (
    SqliteReleaseHistory,
)
from sugarcubes.model_library.infrastructure.enrichment_update_check import (
    ModelEnrichmentUpdateCheck,
)
from sugarcubes.model_library.update_api import register_model_update_routes
from tests.model_library.support.model_updates import NOW, fixture
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease


def test_initial_pull_selection_and_versions_check_without_fabricating_usage(
    tmp_path: Path,
) -> None:
    """Keep calendar freshness durable, version browsing fresh and badges independent of dispatch history."""
    service, entry, provider = fixture(tmp_path)
    store = SqliteReleaseHistory(tmp_path / "history.sqlite3")
    service.releases = replace(service.releases, history_store=store)
    now = NOW
    service.releases.history_cache.clock = lambda: now
    feed = ModelUpdateFeed(service)
    routes = web.RouteTableDef()
    register_model_update_routes(routes, feed)
    app = web.Application()
    app.add_routes(routes)
    path = f"/sugarcubes/v1/models/{entry.evidence.location_id}/updates/check"

    async def exercise() -> None:
        """Drive real HTTP and persistence while isolating the provider network boundary."""
        nonlocal now
        try:
            await ModelEnrichmentUpdateCheck(feed).check(entry.evidence.location_id)
            assert provider.calls == 1
            assert feed.proposals and service.usage.load().enabled
            record = service.usage.load().records[0]
            assert record.usage_count == 0 and record.last_used_at is None
            async with TestClient(TestServer(app)) as client:
                response = await client.post(path)
                assert response.status == 200
                assert (await response.json())["proposals"]
                await client.post(path)
                assert provider.calls == 1
                reopened = replace(service.releases)
                reopened.history_cache.clock = lambda: now
                service.releases = reopened
                await client.post(path)
                assert provider.calls == 1
                now += timedelta(days=1)
                await client.post(path)
                assert provider.calls == 2
                provider.latest = "22"
                family = await feed.browse(service.releases, entry.evidence.location_id)
                assert family.releases[-1].version.version_id == "22"
                assert feed.proposals[0].candidate.version.version_id == "22"
                await client.post(path)
                assert provider.calls == 3
                assert feed.dismiss(entry.evidence.location_id)
                await client.post(path)
                assert not feed.proposals and provider.calls == 3
                assert (
                    await client.post("/sugarcubes/v1/models/unsafe/updates/check")
                ).status == 400
            assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"
            service.record_usage(entry)
            record = service.usage.load().records[0]
            assert record.usage_count == 1 and record.last_used_at == NOW
        finally:
            await feed.close()
            await service.releases.history_cache.close()

    asyncio.run(exercise())


def test_failed_initial_pull_keeps_metadata_and_next_selection_can_retry(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep failure observable without replacing verified catalog metadata or consuming freshness."""
    service, entry, provider = fixture(tmp_path)
    feed = ModelUpdateFeed(service)

    async def exercise() -> None:
        """Fail the release boundary once and recover through the next deliberate selection."""
        original = provider.releases

        async def unavailable(model_id: str) -> tuple[ModelProviderRelease, ...]:
            """Represent a temporary provider outage."""
            raise TimeoutError("offline")

        monkeypatch.setattr(provider, "releases", unavailable)
        await ModelEnrichmentUpdateCheck(feed).check(entry.evidence.location_id)
        assert service.catalog.find_entry(entry.evidence.location_id) == entry
        assert service.usage.load().records == ()
        monkeypatch.setattr(provider, "releases", original)
        await feed.check_model(entry.evidence.location_id)
        assert feed.proposals and provider.calls == 1
        await feed.close()
        await service.releases.history_cache.close()

    asyncio.run(exercise())
