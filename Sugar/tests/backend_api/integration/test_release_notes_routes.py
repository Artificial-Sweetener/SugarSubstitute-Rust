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
"""Prove the release notes routes the page's What's new view reads."""

from __future__ import annotations

import asyncio
import base64
from dataclasses import replace
from pathlib import Path

from sugarcubes.backend.release_notes import ReleaseNotesService
from sugarcubes.backend.routes import build_route_handlers, register_routes
from sugarcubes.backend.self_update.http_client import HttpFailure, HttpFailureKind
from sugarcubes.backend.self_update.release_config import GitHubReleaseConfig
from tests.backend_api.integration.test_self_update_routes import RouteRecorder
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response
from tests.backend_api.support.typing_support import BackendServicesFactory

CONFIG = GitHubReleaseConfig(
    api_root="https://api.github.test",
    repository="example/monorepo",
    tag_prefix="sugarcubes-v",
    asset_name="SugarCubes-{version}.zip",
    download_hosts=frozenset({"github.test"}),
)


class Changelog:
    """Serve one changelog, or fail like an offline computer."""

    def __init__(self, text: str | None) -> None:
        """Keep the changelog; None means offline."""

        self.text = text

    def get_json(
        self, url: str, *, allowed_hosts: frozenset[str], max_bytes: int
    ) -> object:
        """Answer the contents API read."""

        if self.text is None:
            raise HttpFailure(HttpFailureKind.UNREACHABLE, "offline")
        return {
            "encoding": "base64",
            "content": base64.b64encode(self.text.encode()).decode(),
        }

    def download(self, *args: object, **kwargs: object) -> str:
        """Never download anything for release notes."""

        raise AssertionError("release notes never download")


def _folder(path: Path) -> Path:
    path.mkdir()
    return path


def test_the_page_reads_the_running_version_and_its_notes(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """The version route names the product; the notes route answers the range."""

    handlers = build_route_handlers(
        replace(
            backend_services_factory(tmp_path),
            release_notes=ReleaseNotesService(
                config=CONFIG,
                http=Changelog("## [1.1.0](x)\n\n* new\n\n## [1.0.0](x)\n\n* old\n"),
                running_version="1.1.0",
            ),
        )
    )
    version = decode_json_response(
        asyncio.run(handlers.get_release_notes_version(FakeRequest()))
    )
    assert version == {"schema": 1, "product": "SugarCubes", "currentVersion": "1.1.0"}
    notes = decode_json_response(
        asyncio.run(handlers.get_release_notes(FakeRequest(query={"since": "0.9.0"})))
    )
    assert notes == {
        "schema": 1,
        "releases": [
            {"version": "1.1.0", "notes": "## [1.1.0](x)\n\n* new"},
            {"version": "1.0.0", "notes": "## [1.0.0](x)\n\n* old"},
        ],
        "omitted": 0,
    }


def test_offline_and_unpublished_reads_answer_a_failure_and_bad_since_is_refused(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """A failure is an ordinary answer the page shows; a malformed since is a 400."""

    offline = build_route_handlers(
        replace(
            backend_services_factory(_folder(tmp_path / "a")),
            release_notes=ReleaseNotesService(
                config=CONFIG, http=Changelog(None), running_version="1.1.0"
            ),
        )
    )
    answer = decode_json_response(
        asyncio.run(offline.get_release_notes(FakeRequest(query={"since": "1.0.0"})))
    )
    assert answer == {"schema": 1, "failure": "unavailable"}
    unpublished = build_route_handlers(
        replace(
            backend_services_factory(_folder(tmp_path / "b")),
            release_notes=ReleaseNotesService(
                config=CONFIG,
                http=Changelog("## [1.0.0](x)\n"),
                running_version="1.1.0",
            ),
        )
    )
    answer = decode_json_response(
        asyncio.run(
            unpublished.get_release_notes(FakeRequest(query={"since": "1.0.0"}))
        )
    )
    assert answer == {"schema": 1, "failure": "not-published"}
    for query in ({}, {"since": "latest"}):
        assert (
            asyncio.run(offline.get_release_notes(FakeRequest(query=query))).status
            == 400
        )


def test_the_routes_are_registered_on_comfy(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """ComfyUI serves both release notes routes."""

    server = RouteRecorder()
    register_routes(server, backend_services_factory(tmp_path))
    assert ("GET", "/sugarcubes/release-notes") in server.registered
    assert ("GET", "/sugarcubes/release-notes/notes") in server.registered
