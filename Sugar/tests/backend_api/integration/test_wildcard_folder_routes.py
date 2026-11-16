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

"""Prove the route that shows the Comfy user's wildcard folder."""

from __future__ import annotations

import asyncio
import os
from pathlib import Path

from sugarcubes.backend.wildcard_folder_routes import (
    build_open_wildcard_folder_handler,
    register_wildcard_folder_route,
)
from tests.backend_api.integration.test_self_update_routes import RouteRecorder
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response


class UserFiles:
    """Resolve paths inside one user's folder the way ComfyUI's user manager does."""

    def __init__(self, root: Path | None) -> None:
        """Keep the user's folder; None means the request names no known user."""

        self.root = root

    def get_request_user_filepath(
        self, request: object, file: str, *, create_dir: bool = True
    ) -> str | None:
        """Return the path of a file inside the user's folder, creating its parent."""

        if self.root is None:
            raise KeyError("Unknown user")
        path = (self.root / file).resolve()
        if self.root.resolve() not in path.parents:
            return None
        if create_dir:
            os.makedirs(path.parent, exist_ok=True)
        return str(path)


class Opener:
    """Record the folders shown, or fail like a file manager that will not start."""

    def __init__(self, *, available: bool = True, fails: bool = False) -> None:
        """Choose whether this machine has a file manager and whether it starts."""

        self.available = available
        self.fails = fails
        self.opened: list[Path] = []

    def open(self, folder: Path) -> None:
        """Record one shown folder."""

        if self.fails:
            raise OSError("explorer did not start")
        self.opened.append(folder)


def test_the_wildcard_folder_is_created_inside_the_user_folder_and_shown(
    tmp_path: Path,
) -> None:
    opener = Opener()
    handler = build_open_wildcard_folder_handler(UserFiles(tmp_path), opener)

    response = asyncio.run(handler(FakeRequest()))

    folder = tmp_path / "sugar" / "wildcards"
    assert response.status == 200
    assert decode_json_response(response) == {"opened": True}
    assert folder.is_dir()
    assert list(folder.iterdir()) == []
    assert opener.opened == [folder.resolve()]


def test_a_machine_without_a_file_manager_or_a_user_folder_reports_it(
    tmp_path: Path,
) -> None:
    headless = build_open_wildcard_folder_handler(
        UserFiles(tmp_path), Opener(available=False)
    )
    assert asyncio.run(headless(FakeRequest())).status == 501
    assert not (tmp_path / "sugar").exists()

    unknown = build_open_wildcard_folder_handler(UserFiles(None), Opener())
    assert asyncio.run(unknown(FakeRequest())).status == 403

    failing = build_open_wildcard_folder_handler(
        UserFiles(tmp_path), Opener(fails=True)
    )
    response = asyncio.run(failing(FakeRequest()))
    assert response.status == 500
    assert decode_json_response(response)["error"] == {
        "message": "The wildcard folder could not be opened."
    }


def test_the_route_is_registered_only_on_a_server_with_user_files(
    tmp_path: Path,
) -> None:
    server = RouteRecorder()
    register_wildcard_folder_route(server)
    assert server.registered == []

    server.user_manager = UserFiles(tmp_path)  # type: ignore[attr-defined]
    register_wildcard_folder_route(server)
    assert server.registered == [("POST", "/sugarcubes/wildcards/open_folder")]
