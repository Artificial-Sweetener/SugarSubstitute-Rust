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
"""Expose shared provider settings without accepting credentials, paths or arbitrary policy."""

from __future__ import annotations

import asyncio
import logging
import sqlite3

from aiohttp import web

from .application.provider_preferences import ModelProviderPreferenceService
from .infrastructure.provider_preference_codec import (
    provider_preference_patch,
    provider_preferences_payload,
)

_LOGGER = logging.getLogger(__name__)


def register_model_preference_routes(
    routes: web.RouteTableDef, preferences: ModelProviderPreferenceService
) -> None:
    """Translate validated HTTP settings into one authoritative preference transition."""

    async def read(request: web.Request) -> web.Response:
        """Read settings without triggering enrichment, scans or remote lookups."""
        try:
            value = await asyncio.to_thread(preferences.read)
        except (OSError, sqlite3.Error, ValueError):
            _LOGGER.exception("Model provider preference read failed")
            return web.json_response(
                {"error": "Model provider settings are unavailable."}, status=500
            )
        return web.json_response(
            provider_preferences_payload(value), headers={"Cache-Control": "no-store"}
        )

    async def configure(request: web.Request) -> web.Response:
        """Persist only deliberately supplied settings while preserving concurrent edits."""
        try:
            payload: object = await request.json()
            patch = provider_preference_patch(payload)
        except ValueError:
            return web.json_response(
                {"error": "Invalid model provider preferences."}, status=400
            )
        try:
            value = await asyncio.to_thread(preferences.configure, patch)
        except (OSError, sqlite3.Error, ValueError):
            _LOGGER.exception("Model provider preference save failed")
            return web.json_response(
                {"error": "Model provider settings could not be saved."}, status=500
            )
        return web.json_response(
            provider_preferences_payload(value), headers={"Cache-Control": "no-store"}
        )

    routes.get("/sugarcubes/v1/models/preferences")(read)
    routes.patch("/sugarcubes/v1/models/preferences")(configure)
