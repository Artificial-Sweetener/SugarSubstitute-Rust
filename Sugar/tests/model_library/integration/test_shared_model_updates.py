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
"""Prove sibling extensions and browser clients share durable update decisions."""

from __future__ import annotations

import asyncio
from dataclasses import replace
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library import host_api
from sugarcubes.model_library.application.model_update_feed import ModelUpdateFeed
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.update_api import register_model_update_routes
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.model_updates import UpdateProvider, fixture


def test_host_and_http_updates_share_consent_proposals_and_decisions(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exercise both public consumers against one feed and real isolated persistence."""
    service, entry, provider = fixture(tmp_path)
    service.record_usage(entry)
    feed = ModelUpdateFeed(service)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(runtime, "update_feed", feed)
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    routes = web.RouteTableDef()
    register_model_update_routes(routes, feed)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Preserve installed files while observing decisions made by the other frontend."""
        try:
            async with TestClient(TestServer(app)) as client:
                initial = host_api.prepared_model_updates()
                assert initial == {
                    "schemaVersion": 1,
                    "enabled": True,
                    "proposals": [],
                }
                assert (
                    await (await client.get("/sugarcubes/v1/models/updates")).json()
                    == initial
                )
                checked = await host_api.check_model_updates()
                proposals = checked["proposals"]
                assert isinstance(proposals, list)
                assert [
                    {
                        key: proposal[key]
                        for key in ("modelId", "sha256", "modelName", "versionId")
                    }
                    for proposal in proposals
                ] == [
                    {
                        "modelId": entry.evidence.location_id,
                        "sha256": entry.sha256,
                        "modelName": "Fixture",
                        "versionId": "21",
                    }
                ]
                assert (
                    await (await client.get("/sugarcubes/v1/models/updates")).json()
                    == checked
                )
                for unknown in ("", "../private", "https://example.test", "a" * 64):
                    assert not host_api.dismiss_model_update(unknown)
                    assert not host_api.disable_model_updates(unknown)
                assert host_api.prepared_model_updates() == checked
                detached_proposals = checked["proposals"]
                assert isinstance(detached_proposals, list)
                detached_proposals.clear()
                assert host_api.prepared_model_updates()["proposals"]
                assert host_api.dismiss_model_update(entry.evidence.location_id)
                assert not host_api.dismiss_model_update(entry.evidence.location_id)
                assert service.usage.load().records[0].dismissed_version_id == "21"
                assert (
                    await (await client.get("/sugarcubes/v1/models/updates")).json()
                    == host_api.prepared_model_updates()
                )
                provider.latest = "22"
                await client.post("/sugarcubes/v1/models/updates/check")
                assert host_api.prepared_model_updates()["proposals"]
                assert host_api.disable_model_updates(entry.evidence.location_id)
                before = provider.calls
                assert (await host_api.check_model_updates())["proposals"] == []
                assert provider.calls == before
                assert service.usage.load().records[0].updates_disabled_for_model
                await client.put(
                    "/sugarcubes/v1/models/updates", json={"enabled": False}
                )
                assert host_api.prepared_model_updates() == {
                    **initial,
                    "enabled": False,
                }
                assert entry.evidence.value == "installed.safetensors"
                assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"
        finally:
            await feed.close()
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_host_update_operations_reject_an_unavailable_authority(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Fail closed instead of creating desktop-specific stores before host startup."""
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", None)
    for operation in (
        host_api.prepared_model_updates,
        lambda: host_api.set_model_update_notifications(True),
        lambda: host_api.dismiss_model_update("a" * 64),
        lambda: host_api.disable_model_updates("a" * 64),
    ):
        with pytest.raises(RuntimeError, match="not active"):
            operation()
    with pytest.raises(RuntimeError, match="not active"):
        asyncio.run(host_api.check_model_updates())


class DeferredUpdates(UpdateProvider):
    """Control only the provider boundary to prove shared cancellation and recovery."""

    def __init__(self) -> None:
        """Use explicit synchronization rather than timing-dependent provider sleeps."""
        super().__init__()
        self.started = asyncio.Event()
        self.release = asyncio.Event()
        self.unavailable = False

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Pause requests and optionally report a genuine transport timeout."""
        self.started.set()
        await self.release.wait()
        if self.unavailable:
            raise TimeoutError("Deterministic offline provider")
        return await super().releases(model_id)


def test_shared_host_checks_survive_cancelled_clients_and_provider_outages(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    """Keep other consumers and the last snapshot intact while failures remain observable."""
    service, entry, _ = fixture(tmp_path)
    provider = DeferredUpdates()
    service.releases = replace(service.releases, provider=provider)
    service.record_usage(entry)
    feed = ModelUpdateFeed(service)
    feed.set_enabled(True)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(runtime, "update_feed", feed)
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    routes = web.RouteTableDef()
    register_model_update_routes(routes, feed)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Cancel one waiter, fail both transports, then observe a successful fresh retry."""
        try:
            first = asyncio.create_task(host_api.check_model_updates())
            await provider.started.wait()
            second = asyncio.create_task(host_api.check_model_updates())
            first.cancel()
            with pytest.raises(asyncio.CancelledError):
                await first
            provider.release.set()
            snapshot = await second
            assert snapshot["proposals"] and provider.calls == 1
            provider.unavailable = True
            with pytest.raises(TimeoutError, match="offline"):
                await host_api.check_model_updates()
            assert host_api.prepared_model_updates() == snapshot
            async with TestClient(TestServer(app)) as client:
                response = await client.post("/sugarcubes/v1/models/updates/check")
                assert response.status == 502
                assert host_api.prepared_model_updates() == snapshot
            provider.unavailable = False
            provider.latest = "22"
            recovered = await host_api.check_model_updates()
            assert recovered["proposals"] != snapshot["proposals"]
            assert provider.calls == 2
            assert "Model update provider check failed" in caplog.text
        finally:
            await feed.close()
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())
