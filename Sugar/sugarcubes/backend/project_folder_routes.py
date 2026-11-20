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


"""Tell the page where the Comfy user's projects folder is, and reach scripts kept elsewhere.

The page lists, reads and writes project files through ComfyUI's own per-user file
routes. A desktop host's file dialog needs the folder's own path to start in, which
only the machine ComfyUI runs on knows.
"""

from __future__ import annotations

import asyncio
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


SCRIPT_SUFFIX = ".sugar"
"""The one kind of file these routes read or write outside the projects folder."""

MAX_SCRIPT_BYTES = 4 * 1024 * 1024
"""The largest script read or written by its path."""


def _script_path(value: object) -> Path:
    """Return the absolute path of a Sugar script named by the page, or refuse it."""

    if not isinstance(value, str) or not value or "\x00" in value:
        raise BackendError("Name the Sugar script by its full path.", status=400)
    path = Path(value)
    if not path.is_absolute() or path.suffix.casefold() != SCRIPT_SUFFIX:
        raise BackendError(
            "Only a .sugar file can be opened or saved here.", status=400
        )
    return path


def _read_script(path: Path) -> str:
    """Read one script the person chose, bounded and as text."""

    if not path.is_file():
        raise BackendError("The Sugar script no longer exists.", status=404)
    if path.stat().st_size > MAX_SCRIPT_BYTES:
        raise BackendError("The Sugar script is too large to open.", status=413)
    try:
        return path.read_bytes().decode("utf-8-sig")
    except UnicodeDecodeError as error:
        raise BackendError("The file is not a Sugar script.", status=415) from error


def _write_script(path: Path, text: str) -> None:
    """Write one script where the person chose, into a folder that exists."""

    data = text.encode("utf-8")
    if len(data) > MAX_SCRIPT_BYTES:
        raise BackendError("The Sugar script is too large to save.", status=413)
    if not path.parent.is_dir():
        raise BackendError("The folder to save in no longer exists.", status=404)
    path.write_bytes(data)


async def read_external_script(request: Any) -> Any:
    """Read a Sugar script the person chose outside the projects folder."""

    try:
        body = await request.json()
        path = _script_path(body.get("path") if isinstance(body, dict) else None)
        return json_success({"text": await asyncio.to_thread(_read_script, path)})
    except BackendError as error:
        return json_error_from_exception(error)
    except (OSError, ValueError):
        return json_error_from_exception(
            BackendError("The Sugar script could not be read.", status=500)
        )


async def write_external_script(request: Any) -> Any:
    """Write a Sugar script where the person chose outside the projects folder."""

    try:
        body = await request.json()
        fields = body if isinstance(body, dict) else {}
        path = _script_path(fields.get("path"))
        text = fields.get("text")
        if not isinstance(text, str):
            raise BackendError("The Sugar script has no text to save.", status=400)
        await asyncio.to_thread(_write_script, path, text)
        return json_success({"written": True})
    except BackendError as error:
        return json_error_from_exception(error)
    except (OSError, ValueError):
        return json_error_from_exception(
            BackendError("The Sugar script could not be written.", status=500)
        )


def register_project_folder_route(server: Any) -> None:
    """Register the route on a server that has ComfyUI's per-user file manager."""

    user_manager = getattr(server, "user_manager", None)
    if user_manager is None:
        return
    server.routes.get("/sugarcubes/projects/folder")(
        build_project_folder_handler(user_manager)
    )
    server.routes.post("/sugarcubes/projects/script/read")(read_external_script)
    server.routes.post("/sugarcubes/projects/script/write")(write_external_script)


__all__ = [
    "PROJECTS_FOLDER",
    "build_project_folder_handler",
    "read_external_script",
    "write_external_script",
    "register_project_folder_route",
]
