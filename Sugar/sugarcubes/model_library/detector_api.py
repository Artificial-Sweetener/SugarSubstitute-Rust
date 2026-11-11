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
"""Expose detector visual resolution and preferences through bounded text-only requests."""

from __future__ import annotations

import asyncio
import logging
import sqlite3

from aiohttp import web

from .application.detector_service import ModelDetectorService
from .infrastructure.catalog_updates import ModelCatalogUpdates

_LOGGER = logging.getLogger(__name__)


def register_detector_routes(
    routes: web.RouteTableDef,
    service: ModelDetectorService,
    updates: ModelCatalogUpdates,
) -> None:
    """Bind package-art reads and exact-value preferences without accepting filesystem paths."""

    async def library(request: web.Request) -> web.Response:
        """Return every available illustration independently of installed model evidence."""
        try:
            items = await asyncio.to_thread(service.library)
            return web.json_response(
                {
                    "schemaVersion": 1,
                    "revision": 0,
                    "fieldContracts": [],
                    "models": items,
                }
            )
        except (OSError, ValueError):
            _LOGGER.exception("Detector thumbnail library read failed")
            return web.json_response(
                {"error": "Detector thumbnails are unavailable."}, status=500
            )

    async def resolve(request: web.Request) -> web.Response:
        """Validate native choice strings before applying reference presentation policy."""
        try:
            payload: object = await request.json()
        except ValueError:
            return web.json_response({"error": "Invalid detector choices."}, status=400)
        values: object = (
            payload.get("values")
            if isinstance(payload, dict) and set(payload) == {"values"}
            else None
        )
        if (
            not isinstance(values, list)
            or len(values) > 4096
            or any(not isinstance(value, str) or len(value) > 2048 for value in values)
        ):
            return web.json_response({"error": "Invalid detector choices."}, status=400)
        choices = tuple(value for value in values if isinstance(value, str))
        try:
            items = await asyncio.to_thread(service.resolve, choices)
            return web.json_response(
                {
                    "schemaVersion": 1,
                    "revision": 0,
                    "fieldContracts": [],
                    "models": items,
                }
            )
        except (OSError, sqlite3.Error, ValueError):
            _LOGGER.exception("Detector model presentation read failed")
            return web.json_response(
                {"error": "Detector presentation is unavailable."}, status=500
            )

    async def assign(request: web.Request) -> web.Response:
        """Commit only an explicit known-asset preference and publish its catalog revision."""
        try:
            payload: object = await request.json()
        except ValueError:
            return web.json_response(
                {"error": "Invalid detector thumbnail selection."}, status=400
            )
        if (
            not isinstance(payload, dict)
            or set(payload) != {"value", "asset"}
            or not isinstance(payload.get("value"), str)
            or not isinstance(payload.get("asset"), str)
        ):
            return web.json_response(
                {"error": "Invalid detector thumbnail selection."}, status=400
            )
        value, asset = payload["value"], payload["asset"]
        try:
            await asyncio.to_thread(service.assign, value, asset)
            await updates.publish()
            return web.json_response({"schemaVersion": 1})
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Detector thumbnail preference commit failed")
            return web.json_response(
                {"error": "The detector thumbnail could not be saved."}, status=500
            )

    routes.get("/sugarcubes/v1/models/detectors/library")(library)
    routes.post("/sugarcubes/v1/models/detectors/resolve")(resolve)
    routes.put("/sugarcubes/v1/models/detectors/association")(assign)
