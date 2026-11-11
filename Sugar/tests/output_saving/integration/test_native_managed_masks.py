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
"""Prove native mask uploads can read registered external originals without changing saved images."""

from __future__ import annotations

import asyncio
from io import BytesIO
import json
from pathlib import Path

from aiohttp import FormData, web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image
from PIL.PngImagePlugin import PngInfo
import pytest

from sugarcubes.output_saving.infrastructure.http_routes import managed_image_middleware
from sugarcubes.output_saving.infrastructure.runtime import OutputSavingRuntime


@pytest.mark.parametrize("native_failure", [False, True])
def test_managed_mask_original_is_staged_for_native_controls_and_always_removed(
    tmp_path: Path, native_failure: bool
) -> None:
    """Preserve original pixels and recipe metadata while allowing the native mask handler to own its write."""

    async def exercise() -> None:
        """Exercise actual multipart requests through the native image-reference bridge."""
        runtime = OutputSavingRuntime(
            state_root=tmp_path / "state",
            output_root=tmp_path / "output",
            temporary_root=tmp_path / "temp",
        )
        external = tmp_path / "external"
        external.mkdir()
        original = external / "original.png"
        metadata = PngInfo()
        metadata.add_text("workflow", '{"nodes":[]}')
        Image.new("RGB", (4, 3), (10, 20, 30)).save(original, pnginfo=metadata)
        original_bytes = original.read_bytes()
        ref = runtime.artifacts.register(
            root=external, png=original, jpeg=None, saved=True, width=4, height=3
        )
        mask_bytes = BytesIO()
        Image.new("RGBA", (4, 3), (0, 0, 0, 64)).save(mask_bytes, format="PNG")
        observed: list[Path] = []

        async def native_mask(request: web.Request) -> web.Response:
            """Read the adapted reference exactly as Comfy's existing mask-upload closure does."""
            post = await request.post()
            raw = post.get("original_ref")
            assert isinstance(raw, str)
            descriptor = json.loads(raw)
            assert descriptor["type"] == "temp"
            staged = (
                tmp_path / "temp" / descriptor["subfolder"] / descriptor["filename"]
            )
            observed.append(staged)
            assert staged.read_bytes() == original_bytes
            assert post["overwrite"] == "true"
            image = post["image"]
            assert isinstance(image, web.FileField)
            if native_failure:
                raise web.HTTPBadRequest(text="native rejection")
            with Image.open(staged) as source, Image.open(image.file) as mask:
                output = source.convert("RGBA")
                output.putalpha(mask.getchannel("A"))
                assert output.getpixel((0, 0)) == (10, 20, 30, 64)
                assert source.info["workflow"] == '{"nodes":[]}'
                output.save(tmp_path / "native_mask.png")
            return web.json_response(
                {"name": "native_mask.png", "type": "input", "subfolder": ""}
            )

        application = web.Application(middlewares=[managed_image_middleware(runtime)])
        application.router.add_post("/upload/mask", native_mask)
        async with TestClient(TestServer(application)) as client:
            form = FormData()
            form.add_field("original_ref", json.dumps(ref.comfy_image()))
            form.add_field("overwrite", "true")
            form.add_field(
                "image",
                mask_bytes.getvalue(),
                filename="mask.png",
                content_type="image/png",
            )
            response = await client.post("/upload/mask", data=form)
            assert response.status == (400 if native_failure else 200)
            assert len(observed) == 1
            assert not observed[0].exists()
            assert original.read_bytes() == original_bytes

    asyncio.run(exercise())


def test_unregistered_mask_original_fails_before_native_upload(tmp_path: Path) -> None:
    """Never stage an arbitrary server path supplied through a managed-looking image descriptor."""

    async def exercise() -> None:
        """Observe denial at the HTTP boundary without invoking the native mask writer."""
        runtime = OutputSavingRuntime(
            state_root=tmp_path / "state",
            output_root=tmp_path / "output",
            temporary_root=tmp_path / "temp",
        )
        calls: list[str] = []

        async def native_mask(request: web.Request) -> web.Response:
            """Make unrelated delegation observable without reading or writing image files."""
            calls.append("native")
            return web.Response(text="native")

        application = web.Application(middlewares=[managed_image_middleware(runtime)])
        application.router.add_post("/upload/mask", native_mask)
        async with TestClient(TestServer(application)) as client:
            descriptor = {
                "filename": "secret.png",
                "subfolder": "__sugarcubes_managed__/../../elsewhere",
                "type": "output",
            }
            response = await client.post(
                "/upload/mask", data={"original_ref": json.dumps(descriptor)}
            )
            assert response.status == 404
            assert calls == []
            response = await client.post(
                "/upload/mask",
                data={
                    "original_ref": json.dumps(
                        {"filename": "native.png", "type": "output"}
                    )
                },
            )
            assert response.status == 200
            assert calls == ["native"]
            assert not (tmp_path / "temp").exists()

    asyncio.run(exercise())
