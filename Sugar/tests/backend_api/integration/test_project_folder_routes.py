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


"""Prove the page is told where the requesting user's projects folder is."""

from __future__ import annotations

import asyncio
from pathlib import Path

from sugarcubes.backend.project_folder_routes import (
    build_project_folder_handler,
    register_project_folder_route,
)
from tests.backend_api.integration.test_self_update_routes import RouteRecorder
from tests.backend_api.integration.test_wildcard_folder_routes import UserFiles
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response


def test_the_projects_folder_is_created_inside_the_user_folder_and_named(
    tmp_path: Path,
) -> None:
    handler = build_project_folder_handler(UserFiles(tmp_path))

    response = asyncio.run(handler(FakeRequest()))

    folder = tmp_path / "projects"
    assert response.status == 200
    assert decode_json_response(response) == {"folder": str(folder.resolve())}
    assert folder.is_dir()
    assert list(folder.iterdir()) == []


def test_a_request_from_no_known_user_is_told_nothing(tmp_path: Path) -> None:
    unknown = build_project_folder_handler(UserFiles(None))

    response = asyncio.run(unknown(FakeRequest()))

    assert response.status == 403
    assert decode_json_response(response)["error"] == {
        "message": "The projects folder is unavailable."
    }


def test_the_route_is_registered_only_on_a_server_with_user_files(
    tmp_path: Path,
) -> None:
    server = RouteRecorder()
    register_project_folder_route(server)
    assert server.registered == []

    server.user_manager = UserFiles(tmp_path)  # type: ignore[attr-defined]
    register_project_folder_route(server)
    assert server.registered == [("GET", "/sugarcubes/projects/folder")]
