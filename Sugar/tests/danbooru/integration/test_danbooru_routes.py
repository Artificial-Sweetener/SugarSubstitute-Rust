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
"""Prove the pasted-post route against a local Danbooru, including when it fails."""

from __future__ import annotations

import asyncio
from collections import Counter

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.danbooru.api import register_danbooru_routes
from sugarcubes.danbooru.application.danbooru_service import DanbooruService
from sugarcubes.danbooru.infrastructure.danbooru_client import DanbooruClient

POST = {
    "id": 1,
    "md5": "a" * 32,
    "rating": "g",
    "tag_string_general": "long_hair smile",
    "tag_string_artist": "an_artist",
    "tag_string_meta": "highres",
}


def fake_danbooru(calls: Counter[str]) -> web.Application:
    """A Danbooru that knows post 1, fails for post 500 and answers post 7 with nonsense."""

    async def post(request: web.Request) -> web.Response:
        calls["post"] += 1
        post_id = request.match_info["post_id"]
        if post_id == "1":
            return web.json_response(POST)
        if post_id == "500":
            return web.Response(status=503)
        if post_id == "7":
            return web.json_response(["not", "a", "post"])
        return web.Response(status=404)

    async def by_md5(request: web.Request) -> web.Response:
        calls["md5"] += 1
        return web.json_response(POST)

    app = web.Application()
    app.router.add_get("/posts/{post_id}.json", post)
    app.router.add_get("/posts.json", by_md5)
    return app


def test_a_pasted_link_finds_its_post_or_says_why_not() -> None:
    """Found, missing, unavailable and invalid answers reach the editor in its own words."""

    async def main() -> None:
        calls: Counter[str] = Counter()
        async with TestServer(fake_danbooru(calls)) as danbooru:
            client = DanbooruClient(str(danbooru.make_url("")))
            routes = web.RouteTableDef()
            register_danbooru_routes(routes, DanbooruService(client))
            app = web.Application()
            app.add_routes(routes)
            try:
                async with TestClient(TestServer(app)) as http:

                    async def ask(**query: str) -> dict[str, object]:
                        response = await http.get(
                            "/sugarcubes/v1/danbooru/pasted-post", params=query
                        )
                        assert response.headers["Cache-Control"] == "no-store"
                        payload: dict[str, object] = await response.json()
                        return payload

                    found = await ask(id="1")
                    assert found["status"] == "found"
                    assert found["post"] == {
                        "id": 1,
                        "md5": "a" * 32,
                        "tag_string_general": "long_hair smile",
                        "tag_string_artist": "an_artist",
                        "tag_string_copyright": "",
                        "tag_string_character": "",
                        "tag_string_meta": "highres",
                    }
                    assert (await ask(md5="A" * 32))["status"] == "found"
                    assert await ask(id="2") == {"status": "not-found", "post": None}
                    assert (await ask(id="500"))["status"] == "unavailable"
                    assert (await ask(id="7"))["status"] == "invalid"
                    # Text that names no post is never sent to Danbooru.
                    assert (await ask(md5="not-an-md5"))["status"] == "not-found"
                    assert (await ask(id="-3"))["status"] == "not-found"
                    assert (calls["post"], calls["md5"]) == (4, 1)
            finally:
                await client.close()

    asyncio.run(main())
