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

"""Open the Comfy user's wildcard folder in the desktop file manager.

The page lists, reads, creates and renames wildcard files through ComfyUI's own
per-user file routes. Showing the folder is the one thing a browser cannot do,
so this route does it on the machine ComfyUI runs on.
"""

from __future__ import annotations

import asyncio
import os
from pathlib import Path
import subprocess
from typing import Any, Protocol

from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler

WILDCARD_FOLDER = "sugar/wildcards"
"""The wildcard folder inside the Comfy user's own folder."""


class FolderOpener(Protocol):
    """Show one folder in the desktop's file manager."""

    @property
    def available(self) -> bool:
        """Report whether this machine has a desktop file manager to show it in."""

    def open(self, folder: Path) -> None:
        """Show an existing folder."""


class WindowsFolderOpener:
    """Launch Explorer with a fixed executable and the folder as its one argument."""

    def __init__(self) -> None:
        """Discover only Windows' installed Explorer, never a PATH-provided executable."""

        self._executable = Path(os.environ.get("WINDIR", "C:/Windows")) / "explorer.exe"

    @property
    def available(self) -> bool:
        """Disable the action off Windows and in service sessions, which have no desktop."""

        return (
            os.name == "nt"
            and os.environ.get("SESSIONNAME", "").casefold() != "services"
            and self._executable.is_absolute()
            and self._executable.is_file()
        )

    def open(self, folder: Path) -> None:
        """Hand an existing folder to Explorer, whose window handoff returns zero or one."""

        if not self.available or not folder.is_absolute() or not folder.is_dir():
            raise ValueError("The folder cannot be opened.")
        # The executable is the validated Windows Explorer; the folder is its only argument.
        result = subprocess.run(  # noqa: S603
            [str(self._executable), str(folder)],
            check=False,
            timeout=10,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode not in {0, 1}:
            raise OSError("The file manager could not open the folder.")


def build_open_wildcard_folder_handler(
    user_manager: Any, opener: FolderOpener
) -> RouteHandler:
    """Build the handler that creates the requesting user's wildcard folder and shows it."""

    async def open_wildcard_folder(request: Any) -> Any:
        if not opener.available:
            return json_error_from_exception(
                BackendError(
                    "This computer has no file manager to open the folder in.",
                    status=501,
                )
            )
        try:
            # ComfyUI resolves and creates the parent of a path inside the user's folder.
            inside = user_manager.get_request_user_filepath(
                request, f"{WILDCARD_FOLDER}/.folder", create_dir=True
            )
        except KeyError:
            inside = None
        if not inside:
            return json_error_from_exception(
                BackendError("The wildcard folder is unavailable.", status=403)
            )
        folder = Path(inside).parent
        try:
            await asyncio.to_thread(opener.open, folder)
        except (OSError, ValueError, subprocess.TimeoutExpired):
            return json_error_from_exception(
                BackendError("The wildcard folder could not be opened.", status=500)
            )
        return json_success({"opened": True})

    return open_wildcard_folder


def register_wildcard_folder_route(server: Any) -> None:
    """Register the route on a server that has ComfyUI's per-user file manager."""

    user_manager = getattr(server, "user_manager", None)
    if user_manager is None:
        return
    server.routes.post("/sugarcubes/wildcards/open_folder")(
        build_open_wildcard_folder_handler(user_manager, WindowsFolderOpener())
    )


__all__ = [
    "FolderOpener",
    "WILDCARD_FOLDER",
    "WindowsFolderOpener",
    "build_open_wildcard_folder_handler",
    "register_wildcard_folder_route",
]
