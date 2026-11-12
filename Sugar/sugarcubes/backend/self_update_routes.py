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
"""Adapt SugarCubes' own updater to HTTP for the Updates entry.

`GET /sugarcubes/self-update` returns the state. `POST .../check` starts a
background check (`{"force": true}` when the user asks). `POST .../install`
with `{"version": "x.y.z"}` records the choice to install at the next start;
`DELETE .../install` withdraws it. `DELETE .../notice` dismisses the report of
the last start. `POST .../page-load` with `{"loaded": bool}` reports whether the
page loaded SugarCubes, settling a just-installed version. Every reply, and every `sugarcubes.self_update` websocket event,
is the state object described in `self_update.wire`.
"""

from __future__ import annotations

import asyncio
import re
from dataclasses import dataclass
from typing import Any

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from ..nodes import NODE_CLASS_MAPPINGS
from .self_update import SelfUpdateRejected, snapshot_to_wire
from .self_update.comfy_start import comfy_registered_nodes, watch_comfy_start
from .validation import get_bool, parse_optional_json_body

_VERSION = re.compile(r"^[0-9]{1,9}\.[0-9]{1,9}\.[0-9]{1,9}$")


@dataclass(frozen=True)
class SelfUpdateRouteHandlers:
    """Collect the self-update endpoint handlers for registration."""

    get_self_update: RouteHandler
    check_self_update: RouteHandler
    schedule_self_update: RouteHandler
    cancel_self_update: RouteHandler
    dismiss_self_update_notice: RouteHandler
    report_self_update_page_load: RouteHandler


def build_self_update_route_handlers(
    services: BackendServices,
) -> SelfUpdateRouteHandlers:
    """Build thin handlers over the self-update service."""

    updater = services.self_update

    async def get_self_update(request: Any) -> Any:
        _ = request
        return json_success(snapshot_to_wire(updater.snapshot()))

    async def check_self_update(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
        except BackendError as error:
            return json_error_from_exception(error)
        force = get_bool(body or {}, "force", False)
        return json_success(snapshot_to_wire(updater.request_check(force=force)))

    async def schedule_self_update(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
            version = (body or {}).get("version")
            if not isinstance(version, str) or not _VERSION.fullmatch(version):
                raise BackendError("Choose which update to install.", status=400)
            snapshot = await asyncio.to_thread(updater.schedule_install, version)
            return json_success(snapshot_to_wire(snapshot))
        except SelfUpdateRejected as error:
            return json_error_from_exception(BackendError(str(error), status=409))
        except BackendError as error:
            return json_error_from_exception(error)

    async def cancel_self_update(request: Any) -> Any:
        _ = request
        try:
            return json_success(snapshot_to_wire(updater.cancel_install()))
        except SelfUpdateRejected as error:
            return json_error_from_exception(BackendError(str(error), status=409))

    async def report_self_update_page_load(request: Any) -> Any:
        try:
            body = await parse_optional_json_body(request)
        except BackendError as error:
            return json_error_from_exception(error)
        loaded = (body or {}).get("loaded")
        if not isinstance(loaded, bool):
            return json_error_from_exception(
                BackendError("Say whether the page loaded.", status=400)
            )
        await asyncio.to_thread(
            updater.report_page_load,
            loaded=loaded,
            nodes_registered=comfy_registered_nodes(NODE_CLASS_MAPPINGS),
        )
        return json_success(snapshot_to_wire(updater.snapshot()))

    async def dismiss_self_update_notice(request: Any) -> Any:
        _ = request
        return json_success(snapshot_to_wire(updater.dismiss_notice()))

    return SelfUpdateRouteHandlers(
        get_self_update=get_self_update,
        check_self_update=check_self_update,
        schedule_self_update=schedule_self_update,
        cancel_self_update=cancel_self_update,
        dismiss_self_update_notice=dismiss_self_update_notice,
        report_self_update_page_load=report_self_update_page_load,
    )


SELF_UPDATE_EVENT = "sugarcubes.self_update"


def publish_self_update_events(server: Any, services: BackendServices) -> None:
    """Push every updater state change to open pages over ComfyUI's websocket.

    ComfyUI's `send_sync` hands the message to the server loop thread-safely, so
    the updater's worker thread may publish directly.
    """

    send_sync = getattr(server, "send_sync", None)
    if not callable(send_sync):
        return
    services.self_update.subscribe(
        lambda snapshot: send_sync(SELF_UPDATE_EVENT, snapshot_to_wire(snapshot))
    )


def accept_updates_on_full_start(server: Any, services: BackendServices) -> None:
    """Accept a just-installed version only once ComfyUI finishes starting."""

    updater = services.self_update
    observed = watch_comfy_start(
        server,
        expected_nodes=NODE_CLASS_MAPPINGS,
        on_started=lambda registered: updater.accept_backend_start(
            nodes_registered=registered
        ),
    )
    if not observed:
        updater.start_unobservable()


__all__ = [
    "SELF_UPDATE_EVENT",
    "accept_updates_on_full_start",
    "SelfUpdateRouteHandlers",
    "build_self_update_route_handlers",
    "publish_self_update_events",
]
