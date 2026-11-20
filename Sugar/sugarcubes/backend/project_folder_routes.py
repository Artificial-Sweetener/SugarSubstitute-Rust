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


"""Tell the page where the Comfy user's projects folder is on this machine.

The page lists, reads and writes project files through ComfyUI's own per-user file
routes. A desktop host's file dialog needs the folder's own path to start in, which
only the machine ComfyUI runs on knows.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler

PROJECTS_FOLDER = "projects"
"""The projects folder inside the Comfy user's own folder."""


def build_project_folder_handler(user_manager: Any) -> RouteHandler:
    """Build the handler that creates the requesting user's projects folder and names it."""

    async def get_project_folder(request: Any) -> Any:
        try:
            # ComfyUI resolves and creates the parent of a path inside the user's folder.
            inside = user_manager.get_request_user_filepath(
                request, f"{PROJECTS_FOLDER}/.folder", create_dir=True
            )
        except KeyError:
            inside = None
        if not inside:
            return json_error_from_exception(
                BackendError("The projects folder is unavailable.", status=403)
            )
        return json_success({"folder": str(Path(inside).parent)})

    return get_project_folder


def register_project_folder_route(server: Any) -> None:
    """Register the route on a server that has ComfyUI's per-user file manager."""

    user_manager = getattr(server, "user_manager", None)
    if user_manager is None:
        return
    server.routes.get("/sugarcubes/projects/folder")(
        build_project_folder_handler(user_manager)
    )


__all__ = [
    "PROJECTS_FOLDER",
    "build_project_folder_handler",
    "register_project_folder_route",
]
