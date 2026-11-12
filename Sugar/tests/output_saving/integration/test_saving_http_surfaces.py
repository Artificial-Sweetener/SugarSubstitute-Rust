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
"""Prove managed file serving preserves native view controls and validated settings boundaries."""

from __future__ import annotations

import asyncio
from io import BytesIO
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.output_saving.infrastructure.http_routes import (
    managed_image_middleware,
    register_saving_routes,
)
from sugarcubes.output_saving.infrastructure.runtime import OutputSavingRuntime


def test_native_view_channels_and_settings_requests_use_registered_sources(
    tmp_path: Path,
) -> None:
    """Serve exact originals or requested channel previews while delegating unrelated URLs unchanged."""

    async def exercise() -> None:
        """Use a real local HTTP boundary without replacing private server methods."""
        runtime = OutputSavingRuntime(
            state_root=tmp_path / "state",
            output_root=tmp_path / "output",
            temporary_root=tmp_path / "temp",
        )
        root = tmp_path / "output"
        root.mkdir()
        path = root / "result.png"
        Image.new("RGBA", (4, 3), (10, 20, 30, 128)).save(path)
        ref = runtime.artifacts.register(
            root=root, png=path, jpeg=None, saved=True, width=4, height=3
        )
        application = web.Application(middlewares=[managed_image_middleware(runtime)])
        routes = web.RouteTableDef()
        register_saving_routes(routes, runtime)

        async def native_view(request: web.Request) -> web.Response:
            """Expose a distinguishable native handler for delegation assertions."""
            return web.Response(text="native handler")

        routes.get("/view")(native_view)
        application.add_routes(routes)
        async with TestClient(TestServer(application)) as client:
            params = {
                "filename": ref.filename,
                "subfolder": ref.subfolder,
                "type": "output",
            }
            response = await client.get("/view", params=params)
            assert response.status == 200
            assert await response.read() == path.read_bytes()
            response = await client.get("/view", params={**params, "channel": "a"})
            with Image.open(BytesIO(await response.read())) as alpha:
                assert alpha.getpixel((0, 0)) == (0, 0, 0, 128)
            response = await client.get("/view", params={**params, "channel": "rgb"})
            with Image.open(BytesIO(await response.read())) as rgb:
                assert rgb.mode == "RGB"
                assert rgb.getpixel((0, 0)) == (10, 20, 30)
            response = await client.get(
                "/view", params={**params, "preview": "jpeg;80"}
            )
            assert response.content_type == "image/jpeg"
            assert (
                await client.get(
                    "/view", params={**params, "filename": "different.png"}
                )
            ).status == 404
            assert (
                await (
                    await client.get("/view", params={"filename": "ordinary.png"})
                ).text()
                == "native handler"
            )
            response = await client.get("/sugarcubes/output-saving/settings")
            data: object = await response.json()
            assert isinstance(data, dict)
            assert data["success"] is True
            response = await client.post(
                "/sugarcubes/output-saving/transfer", json=params
            )
            transfer: object = await response.json()
            assert response.status == 200
            assert isinstance(transfer, dict)
            assert transfer["data"] == {"file": params, "mime_type": "image/png"}
            response = await client.post(
                "/sugarcubes/output-saving/transfer",
                json={**params, "filename": "different.png"},
            )
            assert response.status == 400
            response = await client.post(
                "/sugarcubes/output-saving/settings", json={"schema_version": 99}
            )
            assert response.status == 400
            assert not (tmp_path / "state" / "preferences.json").exists()

    asyncio.run(exercise())
