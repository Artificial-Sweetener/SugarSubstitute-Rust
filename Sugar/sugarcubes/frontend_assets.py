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
"""Serve deferred frontend modules through Comfy's existing static extension route."""

from __future__ import annotations

from collections.abc import Awaitable, Callable

from aiohttp import web
from aiohttp.typedefs import Middleware

_INSTALLED = web.AppKey("sugarcubes.canvas_module_mime", bool)


def _canvas_module_mime(extension_name: str) -> Middleware:
    """Set JavaScript MIME only for this extension's deferred canvas file responses."""

    prefix = f"/extensions/{extension_name}/comfyui/ui/output_canvas/"

    @web.middleware
    async def set_module_type(
        request: web.Request,
        handler: Callable[[web.Request], Awaitable[web.StreamResponse]],
    ) -> web.StreamResponse:
        response = await handler(request)
        if (
            request.method in {"GET", "HEAD"}
            and request.path.startswith(prefix)
            and request.path.endswith(".mjs")
            and isinstance(response, web.FileResponse)
        ):
            response.content_type = "application/javascript"
        return response

    return set_module_type


def install_canvas_module_mime(prompt_server: object, extension_name: str) -> None:
    """Install the scoped MIME override once on the host's existing application."""

    instance: object = getattr(prompt_server, "instance", prompt_server)
    application: object = getattr(instance, "app", None)
    if not isinstance(application, web.Application):
        raise RuntimeError("Comfy's frontend application is unavailable.")
    if not extension_name or "/" in extension_name or "\\" in extension_name:
        raise ValueError("The extension name is invalid.")
    if application.get(_INSTALLED):
        return
    application.middlewares.append(_canvas_module_mime(extension_name))
    application[_INSTALLED] = True
