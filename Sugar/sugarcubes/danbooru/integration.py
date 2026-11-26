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
"""Install the Danbooru route on ComfyUI's server and close its session on shutdown."""

from __future__ import annotations

from aiohttp import web

from .api import register_danbooru_routes
from .application.danbooru_service import DanbooruService
from .infrastructure.danbooru_client import DanbooruClient


def install_danbooru(prompt_server: object) -> DanbooruService:
    """Compose the service, add its route, and release the session when the server shuts down."""
    instance: object = getattr(prompt_server, "instance", prompt_server)
    routes: object = getattr(instance, "routes", None)
    host_app: object = getattr(instance, "app", None)
    if not isinstance(routes, web.RouteTableDef) or not isinstance(
        host_app, web.Application
    ):
        raise RuntimeError("Comfy's HTTP routes are unavailable.")
    client = DanbooruClient(user_agent="SugarCubes")
    service = DanbooruService(client)
    register_danbooru_routes(routes, service)

    async def close(application: web.Application) -> None:
        """Release the Danbooru session."""
        await client.close()

    host_app.on_cleanup.append(close)
    return service
