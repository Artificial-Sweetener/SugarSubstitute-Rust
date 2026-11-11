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
"""Prove discovery commands retain reviewed offers and use shared observable acquisition state."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_releases import ModelDownloadAccess

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import asyncio
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import dataclass
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library.api import register_model_library_routes
from sugarcubes.model_library.application.model_suggestion_engine import (
    ModelSuggestionEngine,
)
from sugarcubes.model_library.application.model_suggestion_service import (
    ModelSuggestionService,
)
from sugarcubes.model_library.discovery_api import register_model_discovery_routes
from sugarcubes.model_library.infrastructure.discovery_plans import (
    ReviewedDiscoveryPlans,
)
from sugarcubes.model_library.infrastructure.discovery_requests import (
    ModelDiscoveryRequests,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from tests.model_library.support.discovery_http import (
    CONTENT,
    DIGEST,
    DiscoveryProviderFixture,
    DiscoveryPreviewFixture,
)
from tests.model_library.support.enrichment import ObservedProvider, ObservedPreviews
from tests.model_library.support.model_roots import FixtureFileRoots

_BASE = "/sugarcubes/v1/models/discovery"


@dataclass(frozen=True)
class DiscoveryHttpFixture:
    """Retain test-owned boundaries alongside the real HTTP and model authority."""

    client: TestClient[web.Request, web.Application]
    runtime: ModelLibraryRuntime
    public: DiscoveryProviderFixture
    protected: DiscoveryProviderFixture
    previews: DiscoveryPreviewFixture


@asynccontextmanager
async def fixture(root: Path) -> AsyncIterator[DiscoveryHttpFixture]:
    """Close every shared job and HTTP server after deterministic fixture operations."""
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(root, "upscale_models"),
        enrichment_provider=ObservedProvider(),
        enrichment_previews=ObservedPreviews(),
    )
    public = DiscoveryProviderFixture(root, "openmodeldb")
    protected = DiscoveryProviderFixture(
        root, "civitai", ModelDownloadAccess.API_KEY_REQUIRED
    )
    previews = DiscoveryPreviewFixture(runtime.assets)
    requests = ModelDiscoveryRequests(
        ModelSuggestionService(
            runtime.repository, ModelSuggestionEngine((public, protected))
        ),
        previews,
        runtime.artifact_publication,
        runtime.jobs,
        ReviewedDiscoveryPlans(),
    )
    routes = web.RouteTableDef()
    register_model_library_routes(routes, runtime)
    register_model_discovery_routes(
        routes, requests, runtime.assets, model_relationships()
    )
    app = web.Application()
    app.add_routes(routes)
    try:
        async with TestClient(TestServer(app)) as client:
            yield DiscoveryHttpFixture(client, runtime, public, protected, previews)
    finally:
        await runtime.jobs.close()
        await runtime.automatic_enrichment.close()
        await runtime.update_feed.close()
        await runtime.release_previews.close()


async def plan(
    f: DiscoveryHttpFixture, *, public_only: bool = False
) -> dict[str, object]:
    """Observe a completed plan through the same job boundary used by Vue clients."""
    response = await f.client.post(
        _BASE, json={"kind": "upscale_models", "publicOnly": public_only}
    )
    assert response.status == 202
    job_id = (await response.json())["jobId"]
    job = await f.runtime.jobs.wait(job_id)
    assert job is not None and job.status == "completed"
    response = await f.client.get(f"/sugarcubes/v1/models/jobs/{job_id}")
    assert response.status == 200
    payload: dict[str, object] = (await response.json())["result"]
    return payload


def test_http_reviewed_offers_previews_and_exact_publication(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Retain alternate providers, reject arbitrary destinations, and publish only a tiny verified fixture."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)

    async def exercise() -> None:
        """Cross production HTTP, reviewed engine, index, image assets, and job publication boundaries."""
        async with fixture(tmp_path) as f:
            payload = await plan(f)
            session = _BASE + "/" + str(payload["planId"])
            serialized = str(payload)
            assert "download_url" not in serialized and "github.com" not in serialized
            assert str(tmp_path) not in serialized
            suggestions = payload["suggestions"]
            assert isinstance(suggestions, list) and len(suggestions) == 1
            assert suggestions[0]["modelName"] == "<b>Fixture</b>"
            offers = suggestions[0]["offers"]
            assert [offer["providerId"] for offer in offers] == [
                "openmodeldb",
                "civitai",
            ]
            response = await f.client.get(offers[0]["previewUrl"])
            assert (
                response.status == 200
                and response.headers["Content-Type"] == "image/webp"
            )
            assert (await response.read())[:4] == b"RIFF"
            assert f.previews.urls == ["https://image.civitai.com/fixture/image.png"]
            response = await f.client.get(session + "/preview/0/foreign")
            assert response.status == 404
            response = await f.client.get(session + "/preview/999/openmodeldb")
            assert response.status == 404
            assert len(f.previews.urls) == 1
            identity = suggestions[0]["identity"]
            for value in (
                {"identity": identity, "providerId": "foreign"},
                {"identity": "upscale_models:" + "b" * 64, "providerId": "openmodeldb"},
                {
                    "identity": identity,
                    "providerId": "openmodeldb",
                    "url": "https://example.com",
                },
            ):
                response = await f.client.post(session + "/acquire", json=value)
                assert response.status == 400
            denied = await f.client.post(
                session + "/acquire",
                json={"identity": identity, "providerId": "civitai"},
            )
            assert denied.status == 202
            denied_job = await f.runtime.jobs.wait((await denied.json())["jobId"])
            assert denied_job is not None and denied_job.status == "failed"
            assert "API key" in (denied_job.error or "")
            assert not f.public.installs and not f.protected.installs
            public_payload = await plan(f, public_only=True)
            public_cards = public_payload["suggestions"]
            assert isinstance(public_cards, list)
            assert len(public_cards[0]["offers"]) == 1 and public_payload["publicOnly"]
            response = await f.client.post(
                session + "/acquire",
                json={"identity": identity, "providerId": "openmodeldb"},
            )
            assert response.status == 202
            job_id = (await response.json())["jobId"]
            job = await f.runtime.jobs.wait(job_id)
            assert job is not None and job.status == "completed" and not job.cancellable
            assert job.result["fileName"] == "fixture.pth"
            assert job.progress == {
                "bytesDownloaded": len(CONTENT),
                "bytesTotal": len(CONTENT),
            }
            assert (tmp_path / "fixture.pth").read_bytes() == CONTENT
            entry = f.runtime.repository.find_entry(str(job.result["modelId"]))
            assert entry is not None and entry.sha256 == DIGEST
            assert not (await plan(f))["suggestions"]
            response = await f.client.delete(session)
            assert response.status == 204
            response = await f.client.get(offers[0]["previewUrl"])
            assert response.status == 404

    asyncio.run(exercise())


def test_http_context_bounds_and_cancelled_or_offline_planning(tmp_path: Path) -> None:
    """Keep malformed requests out of providers and let loading close without retaining stale choices."""

    async def exercise() -> None:
        """Observe rejected contexts, cancellation, and recoverable errors through real job routes."""
        async with fixture(tmp_path) as f:
            response = await f.client.post(
                _BASE + "/context", json={"kind": "checkpoints"}
            )
            assert response.status == 200 and (await response.json())["context"] is None
            response = await f.client.post(
                _BASE + "/context", json={"kind": "checkpoints", "targetModel": "SDXL"}
            )
            assert (await response.json())["context"] == {
                "artifactKind": "checkpoints",
                "familyId": "sdxl",
            }
            for value in (
                {"kind": "loras", "targetModel": "unrecognized-family"},
                {"kind": 42},
                {"kind": "upscale_models", "publicOnly": "true"},
                {"kind": "upscale_models", "path": "../model"},
            ):
                response = await f.client.post(_BASE, json=value)
                assert response.status == 400
            for content in (b"x" * (16 * 1024 + 1), b"\xff", b"[]"):
                response = await f.client.post(_BASE, data=content)
                assert response.status == 400
            assert not f.public.started.is_set()
            f.public.hold = asyncio.Event()
            response = await f.client.post(_BASE, json={"kind": "upscale_models"})
            job_id = (await response.json())["jobId"]
            await f.public.started.wait()
            response = await f.client.delete(f"/sugarcubes/v1/models/jobs/{job_id}")
            assert response.status == 200
            job = await f.runtime.jobs.wait(job_id)
            assert job is not None and job.status == "cancelled" and not job.result
            f.public.hold = None
            f.public.offline = True
            response = await f.client.post(_BASE, json={"kind": "upscale_models"})
            job = await f.runtime.jobs.wait((await response.json())["jobId"])
            assert job is not None and job.status == "failed" and not job.result
            assert not f.public.installs

    asyncio.run(exercise())


def test_http_transfer_cancellation_and_accepted_offer_survives_closed_picker(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Cancel before publication and retain the exact protected source once its job is accepted."""
    monkeypatch.delenv("CIVITAI_API_KEY", raising=False)
    monkeypatch.delenv("SUGARCUBES_CIVITAI_API_KEY", raising=False)

    async def exercise() -> None:
        """Use inert bytes and synthetic credentials without contacting any external service."""
        async with fixture(tmp_path) as f:
            payload = await plan(f)
            session = _BASE + "/" + str(payload["planId"])
            identity = "upscale_models:" + DIGEST
            f.public.started.clear()
            f.public.hold = asyncio.Event()
            response = await f.client.post(
                session + "/acquire",
                json={"identity": identity, "providerId": "openmodeldb"},
            )
            job_id = (await response.json())["jobId"]
            await f.public.started.wait()
            await f.client.delete(f"/sugarcubes/v1/models/jobs/{job_id}")
            job = await f.runtime.jobs.wait(job_id)
            assert job is not None and job.status == "cancelled"
            assert not (tmp_path / "fixture.pth").exists() and not f.public.installs
            monkeypatch.setenv("SUGARCUBES_CIVITAI_API_KEY", "synthetic-http-key")
            f.protected.started.clear()
            f.protected.hold = asyncio.Event()
            response = await f.client.post(
                session + "/acquire",
                json={"identity": identity, "providerId": "civitai"},
            )
            assert response.status == 202
            job_id = (await response.json())["jobId"]
            await f.protected.started.wait()
            response = await f.client.delete(session)
            assert response.status == 204
            f.protected.hold.set()
            job = await f.runtime.jobs.wait(job_id)
            assert job is not None and job.status == "completed"
            assert not f.public.installs and len(f.protected.installs) == 1
            assert "synthetic-http-key" not in str(job.to_payload())
            assert (tmp_path / "fixture.pth").read_bytes() == CONTENT

    asyncio.run(exercise())
