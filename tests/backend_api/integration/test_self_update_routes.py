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
"""Prove the self-update routes and websocket events the Updates entry uses."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import Any

from sugarcubes.backend.pack_install.startup import run_startup_update
from sugarcubes.backend.routes import build_route_handlers, register_routes
from sugarcubes.backend.self_update import build_self_update_service
from sugarcubes.backend.self_update_routes import SELF_UPDATE_EVENT
from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    InlineJobs,
    decode_json_response,
)
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.pack_install.support.pack_fixture import install_registry_pack, pack_files
from tests.self_update.support.fake_release_web import FakeReleaseWeb


class RecordingServer:
    """Record route registration and websocket messages like PromptServer."""

    def __init__(self) -> None:
        """Start with no routes and no messages."""

        self.registered: list[tuple[str, str]] = []
        self.messages: list[tuple[str, object]] = []
        self.routes = self

    def _record(self, method: str, path: str) -> Callable[[Any], Any]:
        self.registered.append((method, path))
        return lambda handler: handler

    def get(self, path: str) -> Callable[[Any], Any]:
        """Record a GET route."""

        return self._record("GET", path)

    def post(self, path: str) -> Callable[[Any], Any]:
        """Record a POST route."""

        return self._record("POST", path)

    def patch(self, path: str) -> Callable[[Any], Any]:
        """Record a PATCH route."""

        return self._record("PATCH", path)

    def delete(self, path: str) -> Callable[[Any], Any]:
        """Record a DELETE route."""

        return self._record("DELETE", path)

    def send_sync(self, event: str, payload: object) -> None:
        """Record one websocket message."""

        self.messages.append((event, payload))


def _services_with_pack(
    tmp_path: Path, factory: BackendServicesFactory, running: str = "1.0.0"
) -> tuple[Any, Path]:
    pack = tmp_path / "custom_nodes" / "SugarCubes"
    if not pack.exists():
        install_registry_pack(tmp_path, pack_files("1.0.0"))
    web = FakeReleaseWeb()
    web.publish_registry("1.0.0", pack_files("1.0.0"), latest=False)
    web.publish_registry("1.1.0", pack_files("1.1.0"))
    web.publish_empty_github()
    updater = build_self_update_service(
        pack_folder=pack,
        running_version=running,
        http=web,
        clock=lambda: 1,
        runner=InlineJobs(),
    )
    services_root = tmp_path / f"services-{running}"
    services_root.mkdir()
    return replace(factory(services_root), self_update=updater), pack


def test_routes_check_choose_and_withdraw_an_update(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """The page reads state, asks for a check, chooses and withdraws an install."""

    services, _pack = _services_with_pack(tmp_path, backend_services_factory)
    handlers = build_route_handlers(services)
    state = decode_json_response(asyncio.run(handlers.get_self_update(FakeRequest())))
    assert (state["schema"], state["state"]) == (1, "idle")
    checked = decode_json_response(
        asyncio.run(handlers.check_self_update(FakeRequest(body={"force": True})))
    )
    assert checked["state"] == "checking"
    ready = decode_json_response(asyncio.run(handlers.get_self_update(FakeRequest())))
    assert (ready["state"], ready["availableVersion"]) == ("ready", "1.1.0")
    bad = asyncio.run(handlers.schedule_self_update(FakeRequest(body={"version": "x"})))
    assert bad.status == 400
    stale = asyncio.run(
        handlers.schedule_self_update(FakeRequest(body={"version": "2.0.0"}))
    )
    assert stale.status == 409
    chosen = decode_json_response(
        asyncio.run(
            handlers.schedule_self_update(FakeRequest(body={"version": "1.1.0"}))
        )
    )
    assert chosen["state"] == "scheduled"
    withdrawn = decode_json_response(
        asyncio.run(handlers.cancel_self_update(FakeRequest()))
    )
    assert withdrawn["state"] == "ready"
    assert withdrawn["revision"] > ready["revision"]


def test_registration_publishes_changes_and_confirms_a_fresh_install(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Every change reaches open pages, and starting the new version settles it."""

    services, pack = _services_with_pack(tmp_path, backend_services_factory)
    services.self_update.request_check(force=True)
    services.self_update.schedule_install("1.1.0")
    run_startup_update(pack)
    restarted, _pack = _services_with_pack(
        tmp_path, backend_services_factory, running="1.1.0"
    )
    server = RecordingServer()
    register_routes(server, restarted)
    assert ("GET", "/sugarcubes/self-update") in server.registered
    assert ("POST", "/sugarcubes/self-update/install") in server.registered
    events = [
        payload for event, payload in server.messages if event == SELF_UPDATE_EVENT
    ]
    assert events and isinstance(events[-1], dict)
    notice = events[-1]["notice"]
    assert isinstance(notice, dict) and notice["kind"] == "installed"
    assert not (pack / ".sugarcubes" / "updates" / "transaction").exists()
