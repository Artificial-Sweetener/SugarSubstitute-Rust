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
"""Expose optional organizing preferences without accepting filesystem roots."""

from __future__ import annotations

import asyncio
import logging
import sqlite3

from aiohttp import web

from .application.organized_downloads import (
    DownloadPathPreferences,
    validate_download_pattern,
)

_LOGGER = logging.getLogger(__name__)


def register_model_download_routes(
    routes: web.RouteTableDef, preferences: DownloadPathPreferences
) -> None:
    """Translate bounded patterns into the shared download configuration owner."""

    async def read(request: web.Request) -> web.Response:
        """Read settings without triggering file or provider work."""
        try:
            pattern = await asyncio.to_thread(preferences.read)
        except (OSError, sqlite3.Error, ValueError):
            _LOGGER.exception("Model download pattern could not be read")
            return web.json_response(
                {"error": "Download pattern could not be read."}, status=500
            )
        return web.json_response({"schemaVersion": 1, "pattern": pattern})

    async def configure(request: web.Request) -> web.Response:
        """Validate a portable pattern before persisting explicit user intent."""
        try:
            value: object = await request.json()
            if (
                not isinstance(value, dict)
                or set(value) != {"pattern"}
                or not isinstance(value.get("pattern"), str)
            ):
                raise ValueError("Invalid download pattern request.")
            pattern = validate_download_pattern(value["pattern"])
            await asyncio.to_thread(preferences.save, pattern)
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model download pattern could not be saved")
            return web.json_response(
                {"error": "Download pattern could not be saved."}, status=500
            )
        return web.json_response({"schemaVersion": 1, "pattern": pattern})

    routes.get("/sugarcubes/v1/models/download-preferences")(read)
    routes.put("/sugarcubes/v1/models/download-preferences")(configure)
