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
"""Serve bounded source images and effective masks through native Comfy codecs."""

from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable

from aiohttp import web

from ...backend.responses import json_error

from .comfy_mask_preview import native_mask_preview
from .input_preview_settings import InputPreviewSettings, parse_preview_settings
from .comfy_image_preview import native_image_preview

_logger = logging.getLogger(__name__)
_INSTALLED = web.AppKey("sugarcubes.input_canvas", bool)


def install_input_previews(
    prompt_server: object,
    *,
    preview: Callable[[str, InputPreviewSettings, int], bytes] = native_mask_preview,
    image_preview: Callable[
        [str, InputPreviewSettings, int], bytes
    ] = native_image_preview,
) -> None:
    """Install one read-only file interpretation route without changing canvas capabilities."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    application: object = getattr(instance, "app", None)
    routes: object = getattr(instance, "routes", None)
    if not isinstance(application, web.Application) or not isinstance(
        routes, web.RouteTableDef
    ):
        raise RuntimeError("Comfy's canvas-input HTTP surface is unavailable.")
    if application.get(_INSTALLED):
        return
    gate = asyncio.Semaphore(2)

    async def input_preview(request: web.Request) -> web.Response:
        """Decode the selected upload source through its native image or mask codec."""
        try:
            payload: object = await request.json()
            if not isinstance(payload, dict):
                raise ValueError("Invalid mask preview request.")
            file: object = payload.get("file")
            if not isinstance(file, str):
                raise ValueError("Invalid mask source file.")
            settings = parse_preview_settings(payload.get("settings"))
            maximum_size: object = payload.get("maximumSize", 288)
            if type(maximum_size) is not int:
                raise ValueError("Invalid mask preview size.")
            async with gate:
                decoder = image_preview if settings.kind == "image" else preview
                pixels = await asyncio.to_thread(decoder, file, settings, maximum_size)
            return web.Response(
                body=pixels,
                content_type="image/png",
                headers={"Cache-Control": "no-store"},
            )
        except FileNotFoundError as error:
            return web.json_response({"error": str(error)}, status=404)
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except Exception:
            # Installed codecs and custom mask loaders may raise arbitrary native exceptions.
            _logger.exception("SugarCubes: input source preview failed")
            return json_error("The input source preview is unavailable.", status=500)

    routes.post("/sugarcubes/v2/canvas-inputs/preview")(input_preview)
    routes.post("/sugarcubes/v2/canvas-inputs/mask-preview")(input_preview)
    application[_INSTALLED] = True
