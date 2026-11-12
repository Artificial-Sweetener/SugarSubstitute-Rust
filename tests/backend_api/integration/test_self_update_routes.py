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
import sys
import types
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from sugarcubes.backend.pack_install.startup import run_startup_update
from sugarcubes.nodes import NODE_CLASS_MAPPINGS
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
        self.started = False
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

    async def start_multi_address(
        self,
        addresses: list[tuple[str, int]],
        call_on_start: object = None,
        verbose: bool = True,
    ) -> None:
        """Stand in for ComfyUI starting its listeners."""

        self.started = True


def _services_with_pack(
    tmp_path: Path, factory: BackendServicesFactory, running: str = "1.0.0"
) -> tuple[Any, Path]:
    pack = tmp_path / "custom_nodes" / "SugarCubes"
    if not pack.exists():
        install_registry_pack(tmp_path, pack_files("1.0.0"))
    web = FakeReleaseWeb()
    web.publish_registry("1.0.0", pack_files("1.0.0"))
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


def _installed_and_restarted(
    tmp_path: Path, factory: BackendServicesFactory
) -> tuple[Any, Path, RecordingServer]:
    """Install 1.1.0 at a start, then register the new version's routes."""

    services, pack = _services_with_pack(tmp_path, factory)
    services.self_update.request_check(force=True)
    services.self_update.schedule_install("1.1.0")
    run_startup_update(pack)
    restarted, _pack = _services_with_pack(tmp_path, factory, running="1.1.0")
    server = RecordingServer()
    register_routes(server, restarted)
    return restarted, pack, server


def _comfy_nodes(monkeypatch: pytest.MonkeyPatch, *, registered: bool) -> None:
    """Stand in for ComfyUI's node registry after custom nodes imported."""

    module = types.ModuleType("nodes")
    module.__dict__["NODE_CLASS_MAPPINGS"] = (
        dict(NODE_CLASS_MAPPINGS) if registered else {}
    )
    monkeypatch.setitem(sys.modules, "nodes", module)


def test_full_start_and_page_load_accept_a_fresh_install(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The new version is accepted at listening, settled by its first page."""

    restarted, pack, server = _installed_and_restarted(
        tmp_path, backend_services_factory
    )
    transaction = pack / ".sugarcubes" / "updates" / "transaction"
    assert ("POST", "/sugarcubes/self-update/page-load") in server.registered
    assert transaction.exists() and not server.messages
    _comfy_nodes(monkeypatch, registered=True)
    asyncio.run(server.start_multi_address([("127.0.0.1", 8188)]))
    assert server.started
    events = [
        payload for event, payload in server.messages if event == SELF_UPDATE_EVENT
    ]
    assert events and isinstance(events[-1], dict)
    notice = events[-1]["notice"]
    assert isinstance(notice, dict) and notice["kind"] == "installed"
    handlers = build_route_handlers(restarted)
    asyncio.run(
        handlers.report_self_update_page_load(FakeRequest(body={"loaded": True}))
    )
    assert not transaction.exists()
    assert run_startup_update(pack) is None


def test_version_that_fails_after_its_routes_register_rolls_back(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Routes alone prove nothing: unregistered nodes leave the update unaccepted."""

    _restarted, pack, server = _installed_and_restarted(
        tmp_path, backend_services_factory
    )
    _comfy_nodes(monkeypatch, registered=False)
    asyncio.run(server.start_multi_address([("127.0.0.1", 8188)]))
    outcome = run_startup_update(pack)
    assert outcome is not None
    assert (outcome.kind.value, outcome.reason) == ("restored", "did-not-start")
    assert (pack / "__init__.py").read_text(encoding="utf-8") == "VERSION = '1.0.0'\n"


def test_page_reported_load_failure_rolls_back_at_the_next_start(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A page that could not load SugarCubes counts as a failed start."""

    restarted, pack, server = _installed_and_restarted(
        tmp_path, backend_services_factory
    )
    _comfy_nodes(monkeypatch, registered=True)
    asyncio.run(server.start_multi_address([("127.0.0.1", 8188)]))
    handlers = build_route_handlers(restarted)
    bad = asyncio.run(handlers.report_self_update_page_load(FakeRequest(body={})))
    assert bad.status == 400
    asyncio.run(
        handlers.report_self_update_page_load(FakeRequest(body={"loaded": False}))
    )
    outcome = run_startup_update(pack)
    assert outcome is not None
    assert (outcome.kind.value, outcome.reason) == ("restored", "page-did-not-load")
    assert (pack / "__init__.py").read_text(encoding="utf-8") == "VERSION = '1.0.0'\n"
