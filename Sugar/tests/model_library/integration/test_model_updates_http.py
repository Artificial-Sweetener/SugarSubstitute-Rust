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
"""Prove enabled defaults, focus coalescing and exact-offer decisions through the update HTTP boundary."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.application.model_update_feed import ModelUpdateFeed
from sugarcubes.model_library.update_api import register_model_update_routes
from tests.model_library.support.model_updates import fixture, UpdateProvider
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease


def test_update_consent_and_menu_decisions_are_durable(tmp_path: Path) -> None:
    """Publish the enabled default and honor saved disabling before accepting exact-offer decisions."""

    async def exercise() -> None:
        """Use real persistence and HTTP with provider traffic limited to a deterministic fixture."""
        service, entry, provider = fixture(tmp_path)
        service.record_usage(entry)
        feed = ModelUpdateFeed(service)
        routes = web.RouteTableDef()
        register_model_update_routes(routes, feed)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            snapshot = await (await client.get("/sugarcubes/v1/models/updates")).json()
            assert snapshot == {"schemaVersion": 1, "enabled": True, "proposals": []}
            disabled = await client.put(
                "/sugarcubes/v1/models/updates", json={"enabled": False}
            )
            assert (await disabled.json())["enabled"] is False
            await client.post("/sugarcubes/v1/models/updates/check")
            assert provider.calls == 0
            for invalid in (
                {"enabled": 1},
                {"enabled": "true"},
                {"enabled": True, "url": "https://example.test"},
            ):
                assert (
                    await client.put("/sugarcubes/v1/models/updates", json=invalid)
                ).status == 400
            assert (
                await client.put(
                    "/sugarcubes/v1/models/updates", json={"enabled": True}
                )
            ).status == 200
            snapshot = await (
                await client.post("/sugarcubes/v1/models/updates/check")
            ).json()
            assert [
                {
                    key: proposal[key]
                    for key in ("modelId", "sha256", "modelName", "versionId")
                }
                for proposal in snapshot["proposals"]
            ] == [
                {
                    "modelId": entry.evidence.location_id,
                    "sha256": "b" * 64,
                    "modelName": "Fixture",
                    "versionId": "21",
                }
            ]
            assert (
                await client.post(
                    "/sugarcubes/v1/models/" + "a" * 64 + "/updates/dismiss"
                )
            ).status == 404
            snapshot = await (
                await client.post(
                    f"/sugarcubes/v1/models/{entry.evidence.location_id}/updates/dismiss"
                )
            ).json()
            assert snapshot["proposals"] == []
            assert (
                await client.post(
                    f"/sugarcubes/v1/models/{entry.evidence.location_id}/updates/dismiss"
                )
            ).status == 404
            provider.latest = "22"
            await client.post("/sugarcubes/v1/models/updates/check")
            assert (
                await client.post(
                    f"/sugarcubes/v1/models/{entry.evidence.location_id}/updates/disable"
                )
            ).status == 200
            assert service.usage.load().records[0].updates_disabled_for_model
            before = provider.calls
            await client.post("/sugarcubes/v1/models/updates/check")
            assert provider.calls == before
        await feed.close()

    asyncio.run(exercise())


class DeferredUpdateProvider(UpdateProvider):
    """Control a provider response so concurrent user decisions can be proved deterministically."""

    def __init__(self) -> None:
        """Create explicit gates rather than relying on arbitrary sleeps."""
        super().__init__()
        self.started = asyncio.Event()
        self.release = asyncio.Event()

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Pause one source response until the test has changed durable consent."""
        self.started.set()
        await self.release.wait()
        return await super().releases(model_id)


def test_shared_focus_check_does_not_resurrect_disabled_proposals(
    tmp_path: Path,
) -> None:
    """Coalesce every waiting client and reject stale publication after a concurrent consent change."""

    async def exercise() -> None:
        """Use controlled provider gates around the real policy and persistence owners."""
        service, entry, _provider = fixture(tmp_path)
        provider = DeferredUpdateProvider()
        service.releases = replace(service.releases, provider=provider)
        service.record_usage(entry)
        feed = ModelUpdateFeed(service)
        feed.set_enabled(True)
        first = asyncio.create_task(feed.check())
        await provider.started.wait()
        feed.set_enabled(False)
        second = asyncio.create_task(feed.check())
        provider.release.set()
        await asyncio.gather(first, second)
        assert provider.calls == 1
        assert feed.proposals == () and not service.usage.load().enabled
        await feed.close()

    asyncio.run(exercise())
