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
"""Expose configured output settings and serve only registered managed image files."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
import logging

from aiohttp import web
from aiohttp.typedefs import Middleware

from ..domain.path_pattern import SUPPORTED_TOKENS
from ..domain.preference_codec import decode_preferences, encode_preferences
from ..domain.preferences import OutputSavingPreferences
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX
from .runtime import OutputSavingRuntime
from .image_views import ManagedImageViews
from .transfer_references import decode_transfer_reference, encode_transfer
from .media_routes import register_media_routes

_LOGGER = logging.getLogger(__name__)


def register_saving_routes(
    routes: web.RouteTableDef, runtime: OutputSavingRuntime
) -> None:
    """Adapt bounded JSON settings changes into the authoritative saving settings service."""

    def snapshot(preferences: OutputSavingPreferences) -> dict[str, object]:
        """Keep validated settings and their example identical across reads and explicit changes."""
        return {
            "success": True,
            "data": {
                "preferences": encode_preferences(preferences),
                "example_path": runtime.settings.preview(preferences),
                "tokens": [f"{{{token}}}" for token in SUPPORTED_TOKENS],
            },
        }

    async def get_settings(request: web.Request) -> web.Response:
        """Publish validated preferences and a nonmutating example path."""
        try:
            preferences = runtime.settings.load()
            return web.json_response(snapshot(preferences))
        except (OSError, ValueError) as error:
            _LOGGER.exception("Output saving settings could not be read.")
            return _error(error, 500)

    async def save_settings(request: web.Request) -> web.Response:
        """Validate one complete proposed settings snapshot before replacing user data."""
        try:
            value: object = await request.json()
            preferences = runtime.settings.save(decode_preferences(value))
            return web.json_response(snapshot(preferences))
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError as error:
            _LOGGER.exception("Output saving settings could not be written.")
            return _error(error, 500)

    async def import_settings(request: web.Request) -> web.Response:
        """Import supplied Substitute settings without reading arbitrary server filesystem paths."""
        try:
            value: object = await request.json()
            preferences = runtime.settings.import_substitute(value)
            return web.json_response(snapshot(preferences))
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError as error:
            _LOGGER.exception("Substitute output settings could not be imported.")
            return _error(error, 500)

    async def preview_settings(request: web.Request) -> web.Response:
        """Validate a proposed example without writing preferences or creating output directories."""
        try:
            value: object = await request.json()
            return web.json_response(snapshot(decode_preferences(value)))
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError as error:
            _LOGGER.exception("Output saving example could not be read.")
            return _error(error, 500)

    async def get_capabilities(request: web.Request) -> web.Response:
        """Expose qualified saver contracts and clearly identify outputs that retain native saving."""
        return web.json_response(
            {"success": True, "data": {"nodes": runtime.capabilities.catalog()}}
        )

    routes.get("/sugarcubes/output-saving/settings")(get_settings)
    routes.post("/sugarcubes/output-saving/settings")(save_settings)
    routes.post("/sugarcubes/output-saving/import")(import_settings)
    routes.post("/sugarcubes/output-saving/preview")(preview_settings)
    routes.get("/sugarcubes/output-saving/capabilities")(get_capabilities)

    async def choose_transfer(request: web.Request) -> web.Response:
        """Select the exact representation through shared policy without opening arbitrary client paths."""
        try:
            value: object = await request.json()
            selection = runtime.transfers.choose(decode_transfer_reference(value))
            return web.json_response(
                {"success": True, "data": encode_transfer(selection)}
            )
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError as error:
            _LOGGER.exception(
                "The requested output image could not be prepared for transfer."
            )
            return _error(error, 500)

    routes.post("/sugarcubes/output-saving/transfer")(choose_transfer)
    register_media_routes(routes, runtime.media)


def managed_image_middleware(runtime: OutputSavingRuntime) -> Middleware:
    """Preserve native Comfy view URLs while resolving external roots through opaque artifact records."""

    views = ManagedImageViews()

    @web.middleware
    async def serve_registered_image(
        request: web.Request,
        handler: Callable[[web.Request], Awaitable[web.StreamResponse]],
    ) -> web.StreamResponse:
        """Intercept only the exact managed namespace; delegate all other native routes unchanged."""
        if request.method == "POST" and request.path in {
            "/upload/mask",
            "/api/upload/mask",
        }:
            return await runtime.mask_uploads.response(request, handler)
        if request.method not in {"GET", "HEAD"} or request.path not in {
            "/view",
            "/api/view",
        }:
            return await handler(request)
        subfolder = request.query.get("subfolder", "")
        if not subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
            return await handler(request)
        identity = subfolder[len(MANAGED_SUBFOLDER_PREFIX) :]
        path = runtime.artifacts.resolve(
            identity,
            request.query.get("filename", ""),
            request.query.get("type", "output"),
        )
        if path is None:
            raise web.HTTPNotFound(text="This managed image file is unavailable.")
        return await views.response(request, path)

    return serve_registered_image


def _error(error: Exception, status: int) -> web.Response:
    """Keep settings errors structured for the SugarCubes modal and toast presentation owners."""
    return web.json_response(
        {"success": False, "error": {"message": str(error)}}, status=status
    )
