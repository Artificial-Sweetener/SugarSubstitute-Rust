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
"""Verify deferred browser modules are served with a JavaScript MIME type."""

from __future__ import annotations

import asyncio
from pathlib import Path
from types import SimpleNamespace

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.frontend_assets import install_canvas_module_mime


def test_canvas_module_mime_is_scoped_to_the_installed_extension(
    tmp_path: Path,
) -> None:
    """Exercise real aiohttp static responses, including HEAD and unrelated extensions."""

    async def exercise() -> None:
        directory = tmp_path / "assets" / "comfyui" / "ui" / "output_canvas"
        directory.mkdir(parents=True)
        (directory / "renderer.mjs").write_text(
            "export const value = 1;", encoding="utf8"
        )
        (directory / "regular.js").write_text(
            "export const value = 2;", encoding="utf8"
        )
        app = web.Application()
        server = SimpleNamespace(app=app)
        install_canvas_module_mime(server, "SugarCubes")
        install_canvas_module_mime(server, "SugarCubes")
        assert len(app.middlewares) == 1
        app.router.add_static("/extensions/SugarCubes", tmp_path / "assets")

        async def unrelated(_: web.Request) -> web.Response:
            return web.Response(text="unrelated", content_type="text/plain")

        app.router.add_get(
            "/extensions/Other/comfyui/ui/output_canvas/renderer.mjs", unrelated
        )
        async with TestClient(TestServer(app)) as client:
            prefix = "/extensions/SugarCubes/comfyui/ui/output_canvas/"
            response = await client.get(prefix + "renderer.mjs")
            assert response.status == 200
            assert response.content_type == "application/javascript"
            assert await response.text() == "export const value = 1;"
            response = await client.head(prefix + "renderer.mjs")
            assert response.status == 200
            assert response.content_type == "application/javascript"
            response = await client.get(prefix + "regular.js")
            assert response.status == 200
            other = await client.get(
                "/extensions/Other/comfyui/ui/output_canvas/renderer.mjs"
            )
            assert other.content_type == "text/plain"
            assert (await client.get(prefix + "missing.mjs")).status == 404

    asyncio.run(exercise())
