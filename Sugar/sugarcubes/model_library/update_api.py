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
"""Expose durable consent and shared update proposals through a thin HTTP boundary."""

from __future__ import annotations

import logging
import re
import sqlite3

import aiohttp
from aiohttp import web

from .application.model_update_feed import ModelUpdateFeed
from .infrastructure.model_update_payloads import model_update_snapshot

_LOGGER = logging.getLogger(__name__)


def register_model_update_routes(
    routes: web.RouteTableDef, feed: ModelUpdateFeed
) -> None:
    """Register focus checks and exact-offer decisions without accepting model URLs or paths."""

    async def read(request: web.Request) -> web.Response:
        """Read consent and cached proposals without triggering a provider request."""
        return web.json_response(model_update_snapshot(feed))

    async def consent(request: web.Request) -> web.Response:
        """Validate deliberate notification changes before persisting them."""
        try:
            payload: object = await request.json()
        except ValueError:
            return web.json_response(
                {"error": "Invalid model update preference."}, status=400
            )
        if (
            not isinstance(payload, dict)
            or set(payload) != {"enabled"}
            or not isinstance(payload.get("enabled"), bool)
        ):
            return web.json_response(
                {"error": "Invalid model update preference."}, status=400
            )
        feed.set_enabled(payload["enabled"])
        return web.json_response(model_update_snapshot(feed))

    async def check(request: web.Request) -> web.Response:
        """Check one selected model daily or honor an explicit library-wide refresh."""
        location_id = request.match_info.get("model_id")
        if location_id is not None and not re.fullmatch(r"[a-f0-9]{64}", location_id):
            return web.json_response(
                {"error": "Invalid model update identity."}, status=400
            )
        try:
            if location_id is None:
                await feed.check()
            else:
                await feed.check_model(location_id)
        except (aiohttp.ClientError, TimeoutError, ValueError):
            _LOGGER.exception("Model update provider check failed")
            return web.json_response(
                {"error": "Model updates could not be checked. Try again later."},
                status=502,
            )
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model update catalog read failed")
            return web.json_response(
                {"error": "Model update storage is unavailable."}, status=500
            )
        return web.json_response(model_update_snapshot(feed))

    async def decide(request: web.Request) -> web.Response:
        """Apply dismissal or page opt-out only to a currently offered exact catalog identity."""
        model_id = request.match_info["model_id"]
        if not re.fullmatch(r"[a-f0-9]{64}", model_id):
            return web.json_response(
                {"error": "Invalid model update identity."}, status=400
            )
        changed = (
            feed.dismiss(model_id)
            if request.match_info["decision"] == "dismiss"
            else feed.disable_model(model_id)
        )
        if not changed:
            return web.json_response(
                {"error": "The model update is no longer available."}, status=404
            )
        return web.json_response(model_update_snapshot(feed))

    routes.get("/sugarcubes/v1/models/updates")(read)
    routes.put("/sugarcubes/v1/models/updates")(consent)
    routes.post("/sugarcubes/v1/models/updates/check")(check)
    routes.post("/sugarcubes/v1/models/{model_id}/updates/check")(check)
    routes.post("/sugarcubes/v1/models/{model_id}/updates/{decision:dismiss|disable}")(
        decide
    )
