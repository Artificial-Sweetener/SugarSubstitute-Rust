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
from collections.abc import Callable
from pathlib import Path
from typing import Any

from sugarcubes.backend.project_folder_routes import (
    build_project_folder_handler,
    read_external_script,
    register_project_folder_route,
    write_external_script,
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
    assert server.registered == [
        ("GET", "/sugarcubes/projects/folder"),
        ("POST", "/sugarcubes/projects/script/read"),
        ("POST", "/sugarcubes/projects/script/write"),
    ]


class JsonRequest:
    """A request whose body is one JSON value."""

    def __init__(self, body: object) -> None:
        """Keep the body the handler reads."""

        self.body = body

    async def json(self) -> object:
        """Return the body as the handler awaits it."""

        return self.body


def test_a_script_kept_anywhere_is_written_and_read_back_by_its_full_path(
    tmp_path: Path,
) -> None:
    script = tmp_path / "kept elsewhere" / "Demo.sugar"
    script.parent.mkdir()
    text = '# Project: Demo\r\n\r\nuse "a" as A\r\n'

    written = asyncio.run(
        write_external_script(JsonRequest({"path": str(script), "text": text}))
    )
    read = asyncio.run(read_external_script(JsonRequest({"path": str(script)})))

    assert decode_json_response(written) == {"written": True}
    assert script.read_bytes() == text.encode("utf-8")
    assert decode_json_response(read) == {"text": text}


def test_only_a_sugar_script_named_by_its_full_path_is_reached(tmp_path: Path) -> None:
    other = tmp_path / "notes.txt"
    other.write_text("private", encoding="utf-8")
    missing = tmp_path / "gone" / "Demo.sugar"

    def status(handler: Callable[[Any], Any], body: object) -> int:
        return int(asyncio.run(handler(JsonRequest(body))).status)

    assert status(read_external_script, {"path": str(other)}) == 400
    assert status(read_external_script, {"path": "Demo.sugar"}) == 400
    assert status(read_external_script, {"path": 7}) == 400
    assert status(read_external_script, []) == 400
    assert status(read_external_script, {"path": str(tmp_path / "none.sugar")}) == 404
    assert status(write_external_script, {"path": str(other), "text": "x"}) == 400
    assert status(write_external_script, {"path": str(missing), "text": "x"}) == 404
    assert status(write_external_script, {"path": str(tmp_path / "a.sugar")}) == 400
    assert other.read_text(encoding="utf-8") == "private"
