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
"""Verify the effective mask endpoint returns opaque coverage and explicit source failures."""

from __future__ import annotations

import asyncio
from io import BytesIO
from types import SimpleNamespace

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.input_canvas.infrastructure.input_preview_settings import (
    InputPreviewSettings,
)
from sugarcubes.input_canvas.infrastructure.http_routes import install_input_previews


def test_mask_preview_http_preserves_effective_output_and_source_error_statuses() -> (
    None
):
    """Exercise the registered route and HTTP transport instead of invoking a private handler."""
    calls: list[tuple[str, str, int]] = []

    def preview(file: str, settings: InputPreviewSettings, maximum_size: int) -> bytes:
        """Replace the external native decoder while retaining real PNG response bytes."""
        channel = str(settings.inputs["channel"])
        calls.append((file, channel, maximum_size))
        if file == "missing.png":
            raise FileNotFoundError("The mask source file is unavailable.")
        if channel == "invalid":
            raise ValueError("Unsupported mask channel.")
        if file == "broken.png":
            raise ZeroDivisionError("Native decoder failed.")
        output = BytesIO()
        Image.new("L", (3, 2), 127).save(output, format="PNG")
        return output.getvalue()

    async def run() -> None:
        """Inspect successful PNG output, validation and native-boundary failure responses."""
        app = web.Application()
        routes = web.RouteTableDef()
        server = SimpleNamespace(app=app, routes=routes)
        install_input_previews(server, preview=preview)
        install_input_previews(server, preview=preview)
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.post(
                "/sugarcubes/v2/canvas-inputs/mask-preview",
                json={
                    "file": "source.png",
                    "settings": {
                        "classType": "LoadImageMask",
                        "field": "image",
                        "inputs": {"channel": "green"},
                    },
                    "maximumSize": 288,
                },
            )
            assert response.status == 200
            assert response.content_type == "image/png"
            assert response.headers["Cache-Control"] == "no-store"
            image = Image.open(BytesIO(await response.read()))
            assert image.size == (3, 2)
            assert set(image.tobytes()) == {127}
            for file, channel, status in [
                ("missing.png", "red", 404),
                ("source.png", "invalid", 400),
                ("broken.png", "red", 500),
            ]:
                failure = await client.post(
                    "/sugarcubes/v2/canvas-inputs/mask-preview",
                    json={
                        "file": file,
                        "settings": {
                            "classType": "LoadImageMask",
                            "field": "image",
                            "inputs": {"channel": channel},
                        },
                    },
                )
                assert failure.status == status
                assert failure.content_type == "application/json"
            assert calls[0] == ("source.png", "green", 288)

    asyncio.run(run())


def test_image_source_route_selects_its_codec_without_running_mask_interpretation() -> (
    None
):
    """Exercise the shared endpoint with real RGBA response bytes and validated image settings."""
    calls: list[InputPreviewSettings] = []

    def image_preview(
        file: str, settings: InputPreviewSettings, maximum_size: int
    ) -> bytes:
        """Replace the native image decoder at the external worker boundary."""
        assert file == "source.png" and maximum_size == 288
        calls.append(settings)
        output = BytesIO()
        Image.new("RGBA", (3, 2), (10, 20, 30, 127)).save(output, format="PNG")
        return output.getvalue()

    async def run() -> None:
        """Use the installed route and its actual JSON/PNG transport."""
        app = web.Application()
        routes = web.RouteTableDef()
        install_input_previews(
            SimpleNamespace(app=app, routes=routes), image_preview=image_preview
        )
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.post(
                "/sugarcubes/v2/canvas-inputs/preview",
                json={
                    "file": "source.png",
                    "maximumSize": 288,
                    "settings": {
                        "kind": "image",
                        "classType": "LoadImage",
                        "field": "image",
                        "inputs": {},
                    },
                },
            )
            assert response.status == 200 and response.content_type == "image/png"
            image = Image.open(BytesIO(await response.read()))
            assert image.mode == "RGBA" and image.size == (3, 2)
            assert image.getpixel((0, 0)) == (10, 20, 30, 127)
            invalid = await client.post(
                "/sugarcubes/v2/canvas-inputs/preview",
                json={"settings": {"kind": "video"}},
            )
            assert invalid.status == 400
        assert len(calls) == 1 and calls[0].kind == "image"

    asyncio.run(run())
