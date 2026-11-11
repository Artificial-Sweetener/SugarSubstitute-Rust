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
"""Expose provider usage and maintenance through the runtime's shared job owner."""

from __future__ import annotations

import logging
import sqlite3

from aiohttp import web

from .infrastructure.runtime import ModelLibraryRuntime

_LOGGER = logging.getLogger(__name__)


def register_model_cache_routes(
    routes: web.RouteTableDef, runtime: ModelLibraryRuntime
) -> None:
    """Translate bounded provider actions without embedding persistence policy in HTTP handlers."""

    async def read(request: web.Request) -> web.Response:
        """Return current usage after prior model image publication finishes."""
        try:
            summary = await runtime.civitai_cache.summary()
        except (OSError, sqlite3.Error, ValueError):
            _LOGGER.exception("CivitAI cache usage read failed")
            return web.json_response(
                {"error": "Model cache usage is unavailable."}, status=500
            )
        return web.json_response(
            summary.to_payload(), headers={"Cache-Control": "no-store"}
        )

    async def start(request: web.Request) -> web.Response:
        """Queue a validated action; existing job routes own observation and cancellation."""
        try:
            job = runtime.cache_actions.start(request.match_info["action"])
        except ValueError:
            return web.json_response(
                {"error": "Unknown model cache action."}, status=400
            )
        return web.json_response(
            job.to_payload(), status=202, headers={"Cache-Control": "no-store"}
        )

    routes.get("/sugarcubes/v1/models/cache/civitai")(read)
    routes.post("/sugarcubes/v1/models/cache/civitai/{action}")(start)
