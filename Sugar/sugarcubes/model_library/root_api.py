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
"""Expose shared model-root configuration through thin host HTTP handlers."""

from __future__ import annotations

import asyncio
import logging

from aiohttp import web

from sugarcubes_model_roots import ModelRootConfiguration

_LOGGER = logging.getLogger(__name__)


def register_model_root_routes(
    routes: web.RouteTableDef, configuration: ModelRootConfiguration
) -> None:
    """Keep browser and Substitute configuration requests on the same authoritative policy."""

    async def status(request: web.Request) -> web.Response:
        """Read persisted and active state without changing the running host."""
        try:
            result = await asyncio.to_thread(configuration.get_status)
            return web.json_response(result.to_payload())
        except (OSError, ValueError, RuntimeError):
            _LOGGER.exception("Model-root status could not be read")
            return web.json_response(
                {"error": "Model-root configuration could not be read."}, status=500
            )

    async def configure(request: web.Request) -> web.Response:
        """Persist validated next-launch intent without moving files or restarting Comfy."""
        try:
            body: object = await request.json()
            if not isinstance(body, dict):
                raise ValueError("Model-root configuration must be an object.")
            mode: object = body.get("mode")
            path: object = body.get("path")
            if not isinstance(mode, str) or (
                path is not None and not isinstance(path, str)
            ):
                raise ValueError("Model-root mode and path are invalid.")
            result = await asyncio.to_thread(configuration.configure, mode, path)
            return web.json_response(result.to_payload())
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except OSError:
            _LOGGER.exception("Model-root configuration could not be saved")
            return web.json_response(
                {"error": "Model-root configuration could not be saved."}, status=500
            )

    routes.get("/sugarcubes/v1/models/root")(status)
    routes.put("/sugarcubes/v1/models/root")(configure)
