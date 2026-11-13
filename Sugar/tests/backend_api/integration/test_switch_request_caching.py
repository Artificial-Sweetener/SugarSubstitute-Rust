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

"""Prove workflow-switch requests revalidate cheaply and never stall the event loop."""

from __future__ import annotations

import asyncio
import dataclasses
import json
import threading
from pathlib import Path
from typing import Any

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.backend.cube_routes import build_cube_route_handlers
from sugarcubes.backend.workflow_library_routes import (
    build_workflow_library_route_handlers,
)
from sugarcubes.library import CatalogCubeArtifact, CubeLibraryClassService
from sugarcubes.library.captured_repository import CapturedCubeRepository
from tests.backend_api.support.backend_fixtures import FakeRequest
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.workflow.support.workflow_fixtures import cube_workflow
from tests.support.no_progress import wait_for_event

CUBE_ID = "Artificial-Sweetener/Base-Cubes/demo.cube"


def _install_icon_cube(services: Any, icon: bytes) -> Path:
    """Write one tracked cube declaring an icon asset and return the icon path."""

    checkout: Path = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    icon_path = checkout / "assets" / "icons" / "demo.png"
    icon_path.parent.mkdir(parents=True, exist_ok=True)
    icon_path.write_bytes(icon)
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": CUBE_ID,
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "Demo",
                    "icon": {"kind": "asset", "path": "assets/icons/demo.png"},
                },
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": {},
                    "definitions": {},
                    "subgraphs": [],
                },
                "surface": {"default_flavor_id": "default", "controls": []},
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )
    return icon_path


def _serve_icon(services: Any, headers: dict[str, str] | None = None) -> Any:
    """Invoke the icon route the way aiohttp delivers one browser request."""

    request = FakeRequest(query={"cube_id": CUBE_ID}, headers=headers)
    return asyncio.run(build_cube_route_handlers(services).serve_icon_asset(request))


def test_icon_revalidates_unchanged_bytes_without_a_transfer(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    _install_icon_cube(services, b"\x89PNG\r\n\x1a\nicon")

    first = _serve_icon(services)
    etag = first.headers["ETag"]
    second = _serve_icon(services)
    revalidated = _serve_icon(services, {"If-None-Match": etag})
    weak = _serve_icon(services, {"If-None-Match": f'"other", W/{etag}'})

    assert first.status == 200
    assert first.body == b"\x89PNG\r\n\x1a\nicon"
    assert first.headers["Cache-Control"] == "no-cache"
    assert etag.startswith('"sha256-') and second.headers["ETag"] == etag
    assert revalidated.status == 304 and revalidated.body is None
    assert revalidated.headers["ETag"] == etag
    assert weak.status == 304


def test_icon_content_change_issues_a_new_validator(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    icon = _install_icon_cube(services, b"\x89PNG\r\n\x1a\nold")
    stale = _serve_icon(services).headers["ETag"]
    icon.write_bytes(b"\x89PNG\r\n\x1a\nnew")

    response = _serve_icon(services, {"If-None-Match": stale})

    assert response.status == 200
    assert response.body == b"\x89PNG\r\n\x1a\nnew"
    assert response.headers["ETag"] != stale


def _artifact(semantic_hash: str, source_ref: str) -> CatalogCubeArtifact:
    """Build one installed candidate for the fixture workflow's definition."""

    return CatalogCubeArtifact(
        cube_id="Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
        cube_version="1.2.0",
        semantic_hash=semantic_hash,
        library_class="synced",
        access="read_only",
        source_ref=source_ref,
    )


def _classify(base: Any, service: CubeLibraryClassService) -> dict[str, Any]:
    """Classify the fixture workflow through the HTTP adapter."""

    handlers = build_workflow_library_route_handlers(
        dataclasses.replace(base, workflow_library=service)
    )
    response = asyncio.run(
        handlers.classify_workflow(FakeRequest(body={"workflow": cube_workflow()}))
    )
    assert response.status == 200
    payload: dict[str, Any] = json.loads(response.body.decode("utf-8"))
    return payload


def test_classification_revision_follows_catalog_content(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    base = backend_services_factory(tmp_path)
    artifacts = [_artifact("a" * 64, "github:Artificial-Sweetener/Base-Cubes:x.cube")]
    service = CubeLibraryClassService(
        captured=CapturedCubeRepository(tmp_path / "captured"),
        catalog_artifacts=lambda: tuple(artifacts),
    )

    first = _classify(base, service)
    repeated = _classify(base, service)
    artifacts.append(_artifact("b" * 64, "local:personal:x.cube"))
    added = _classify(base, service)
    artifacts.pop()
    removed = _classify(base, service)
    artifacts[0] = _artifact("c" * 64, "github:Artificial-Sweetener/Base-Cubes:x.cube")
    changed = _classify(base, service)

    assert first["catalog_revision"].startswith("sha256:")
    assert repeated["catalog_revision"] == first["catalog_revision"]
    assert added["catalog_revision"] != first["catalog_revision"]
    assert removed["catalog_revision"] == first["catalog_revision"]
    assert changed["catalog_revision"] not in {
        first["catalog_revision"],
        added["catalog_revision"],
    }


def test_classification_runs_off_the_event_loop(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    base = backend_services_factory(tmp_path)
    started = threading.Event()
    release = threading.Event()

    def slow_catalog() -> tuple[CatalogCubeArtifact, ...]:
        started.set()
        wait_for_event(release)
        return ()

    service = CubeLibraryClassService(
        captured=CapturedCubeRepository(tmp_path / "captured"),
        catalog_artifacts=slow_catalog,
    )
    handlers = build_workflow_library_route_handlers(
        dataclasses.replace(base, workflow_library=service)
    )

    async def ping(_request: web.Request) -> web.Response:
        return web.Response(text="pong")

    async def run() -> None:
        app = web.Application()
        app.router.add_post("/classify", handlers.classify_workflow)
        app.router.add_get("/ping", ping)
        async with TestClient(TestServer(app)) as client:
            classify = asyncio.create_task(
                client.post("/classify", json={"workflow": cube_workflow()})
            )
            await asyncio.to_thread(wait_for_event, started)
            response = await client.get("/ping")
            assert await response.text() == "pong"
            assert not classify.done()
            release.set()
            classified = await classify
            assert classified.status == 200

    try:
        asyncio.run(run())
    finally:
        release.set()
