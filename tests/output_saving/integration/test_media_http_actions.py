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
"""Exercise guarded file actions and transient image responses over real local HTTP."""

import asyncio
from collections.abc import Sequence
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.output_saving.application.media_service import OutputMediaService
from sugarcubes.output_saving.domain.preferences import OutputSavingPreferences
from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.media_files import HostOutputImageFiles
from sugarcubes.output_saving.infrastructure.media_routes import register_media_routes
from sugarcubes.output_saving.infrastructure.photoshop_opener import PhotoshopLayer


@dataclass
class Revealer:
    """Capture only desktop calls while HTTP, containment and JPEG encoding remain real."""

    paths: list[Path]

    @property
    def available(self) -> bool:
        """Expose a test desktop without starting Explorer during isolated HTTP tests."""
        return True

    def reveal(self, path: Path) -> None:
        """Record the selected validated output."""
        self.paths.append(path)


def test_local_reveal_remote_refusal_and_on_demand_jpeg(tmp_path: Path) -> None:
    """Require same-origin loopback reveal while allowing remote full-resolution downloads."""

    async def exercise() -> None:
        """Use a real request boundary and substitute only the file-manager process."""
        output = tmp_path / "output"
        output.mkdir()
        png = output / "sample.png"
        with Image.new("RGBA", (8, 6), (20, 40, 60, 100)) as image:
            image.save(png)
        revealed: list[Path] = []
        service = OutputMediaService(
            OutputSavingPreferences,
            HostOutputImageFiles(
                ManagedArtifactRegistry(tmp_path / "state.sqlite"),
                {"output": output},
                Revealer(revealed),
            ),
        )
        routes = web.RouteTableDef()
        register_media_routes(routes, service)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            origin = str(client.make_url("/")).rstrip("/")
            reference = {"filename": png.name, "subfolder": "", "type": "output"}
            profile = await (
                await client.get("/sugarcubes/output-saving/media-profile")
            ).json()
            assert profile["data"] == {
                "preferred_copy_format": "png",
                "reveal": True,
                "photoshop": False,
            }
            remote = await (
                await client.get(
                    "/sugarcubes/output-saving/media-profile",
                    headers={"Host": "remote.example"},
                )
            ).json()
            assert remote["data"]["reveal"] is False
            for headers in [
                {},
                {"Origin": "https://attacker.example"},
                {"Origin": origin, "X-Forwarded-For": "203.0.113.1"},
                {"Origin": origin, "Sec-Fetch-Site": "cross-site"},
            ]:
                response = await client.post(
                    "/sugarcubes/output-saving/reveal", json=reference, headers=headers
                )
                assert response.status == 403
            response = await client.post(
                "/sugarcubes/output-saving/reveal",
                json=reference,
                headers={"Origin": origin},
            )
            assert response.status == 200
            assert revealed == [png.resolve()]
            response = await client.post(
                "/sugarcubes/output-saving/reveal",
                json={**reference, "filename": "missing.png"},
                headers={"Origin": origin},
            )
            assert response.status == 400
            video = output / "clip.mp4"
            video.write_bytes(b"\x00\x00\x00\x18ftypmp42")
            video_reference = {**reference, "filename": video.name}
            response = await client.post(
                "/sugarcubes/output-saving/reveal",
                json=video_reference,
                headers={"Origin": origin},
            )
            assert response.status == 200
            assert revealed == [png.resolve(), video.resolve()]
            (tmp_path / "outside.mp4").write_bytes(b"outside")
            for escaping in [
                {**reference, "filename": "outside.mp4", "subfolder": ".."},
                {**reference, "filename": "..\\outside.mp4"},
                {**reference, "filename": str(tmp_path / "outside.mp4")},
            ]:
                response = await client.post(
                    "/sugarcubes/output-saving/reveal",
                    json=escaping,
                    headers={"Origin": origin},
                )
                assert response.status == 400
            assert revealed == [png.resolve(), video.resolve()]
            response = await client.post(
                "/sugarcubes/output-saving/image",
                json={"file": video_reference, "format": "preferred"},
            )
            assert response.status == 400
            response = await client.post(
                "/sugarcubes/output-saving/image",
                json={"file": reference, "format": "jpeg"},
            )
            assert response.status == 200
            assert response.content_type == "image/jpeg"
            assert response.headers["X-SugarCubes-Filename"] == "sample.jpg"
            assert response.headers["Cache-Control"] == "no-store"
            with Image.open(BytesIO(await response.read())) as jpeg:
                assert jpeg.size == (8, 6)
                assert jpeg.mode == "RGB"
            response = await client.get(
                "/sugarcubes/output-saving/image",
                params={**reference, "format": "png"},
                headers={"Host": "remote.example"},
            )
            assert await response.read() == png.read_bytes()
            assert response.headers["Content-Disposition"].startswith("attachment;")
            for format in ["gif", "bad", 100]:
                response = await client.post(
                    "/sugarcubes/output-saving/image",
                    json={"file": reference, "format": format},
                )
                assert response.status == 400
            assert not list(output.glob("*.jpg"))

    asyncio.run(exercise())


@dataclass
class Photoshop:
    """Capture only the Photoshop handoff while HTTP and output containment remain real."""

    opened: list[tuple[str, list[PhotoshopLayer]]]

    @property
    def available(self) -> bool:
        """Expose an installed Photoshop without starting it during isolated HTTP tests."""
        return True

    def open(self, document: str, layers: Sequence[PhotoshopLayer]) -> None:
        """Record the validated document request."""
        self.opened.append((document, list(layers)))


def test_photoshop_opens_contained_still_images_only_for_local_same_origin(
    tmp_path: Path,
) -> None:
    """Publish the capability locally, refuse remote callers, and resolve only contained images."""

    async def exercise() -> None:
        """Use a real request boundary and substitute only the Photoshop process."""
        output = tmp_path / "output"
        output.mkdir()
        images = [output / "first.png", output / "second.png"]
        for path in images:
            with Image.new("RGB", (4, 4)) as image:
                image.save(path)
        (output / "clip.mp4").write_bytes(b"\x00\x00\x00\x18ftypmp42")
        opened: list[tuple[str, list[PhotoshopLayer]]] = []
        service = OutputMediaService(
            OutputSavingPreferences,
            HostOutputImageFiles(
                ManagedArtifactRegistry(tmp_path / "state.sqlite"),
                {"output": output},
                Revealer([]),
                Photoshop(opened),
            ),
        )
        routes = web.RouteTableDef()
        register_media_routes(routes, service)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            origin = str(client.make_url("/")).rstrip("/")
            profile = await (
                await client.get("/sugarcubes/output-saving/media-profile")
            ).json()
            assert profile["data"]["photoshop"] is True
            remote = await (
                await client.get(
                    "/sugarcubes/output-saving/media-profile",
                    headers={"Host": "remote.example"},
                )
            ).json()
            assert remote["data"]["photoshop"] is False

            def layer(filename: str, name: str) -> dict[str, object]:
                """Address one output by its Comfy reference."""
                return {
                    "file": {"filename": filename, "subfolder": "", "type": "output"},
                    "name": name,
                }

            request = {
                "document": "first",
                "layers": [layer("first.png", "Save 1"), layer("second.png", "Save 2")],
            }
            for headers in [{}, {"Origin": "https://attacker.example"}]:
                response = await client.post(
                    "/sugarcubes/output-saving/photoshop", json=request, headers=headers
                )
                assert response.status == 403
            response = await client.post(
                "/sugarcubes/output-saving/photoshop",
                json=request,
                headers={"Origin": origin},
            )
            assert response.status == 200
            assert opened == [
                (
                    "first",
                    [
                        PhotoshopLayer(images[0].resolve(), "Save 1"),
                        PhotoshopLayer(images[1].resolve(), "Save 2"),
                    ],
                )
            ]
            (tmp_path / "outside.png").write_bytes(b"outside")
            for invalid in [
                {"document": "clip", "layers": [layer("clip.mp4", "Video")]},
                {"document": "gone", "layers": [layer("missing.png", "Gone")]},
                {"document": "out", "layers": [layer("..\\outside.png", "Out")]},
                {"document": "named", "layers": [{"file": request["layers"]}]},
                {"layers": []},
            ]:
                response = await client.post(
                    "/sugarcubes/output-saving/photoshop",
                    json=invalid,
                    headers={"Origin": origin},
                )
                assert response.status == 400
            assert len(opened) == 1

    asyncio.run(exercise())


def test_previews_are_small_cacheable_webp_without_touching_the_output(
    tmp_path: Path,
) -> None:
    """Serve a downscaled preview so chrome mirroring an output never decodes it in full."""

    async def exercise() -> None:
        """Encode a large PNG and JPEG through the real route and file containment."""
        output = tmp_path / "output"
        output.mkdir()
        with Image.new("RGB", (4096, 2048), (200, 120, 40)) as image:
            image.save(output / "wide.png")
            image.save(output / "wide.jpg", quality=90)
        original = (output / "wide.png").read_bytes()
        service = OutputMediaService(
            OutputSavingPreferences,
            HostOutputImageFiles(
                ManagedArtifactRegistry(tmp_path / "state.sqlite"),
                {"output": output},
                Revealer([]),
            ),
        )
        routes = web.RouteTableDef()
        register_media_routes(routes, service)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            for name in ["wide.png", "wide.jpg"]:
                response = await client.get(
                    "/sugarcubes/output-saving/preview",
                    params={
                        "filename": name,
                        "subfolder": "",
                        "type": "output",
                        "revision": "result:1",
                        "size": "320",
                    },
                )
                assert response.status == 200
                assert response.content_type == "image/webp"
                assert response.headers["Cache-Control"] == "private, max-age=86400"
                with Image.open(BytesIO(await response.read())) as preview:
                    assert preview.size == (320, 160)
                    pixel = preview.convert("RGB").getpixel((160, 80))
                    assert isinstance(pixel, tuple)
                    red, green, blue = pixel[:3]
                    assert abs(red - 200) < 8 and abs(green - 120) < 8
                    assert abs(blue - 40) < 8
            reference = {"filename": "wide.png", "subfolder": "", "type": "output"}
            for size in ["", "32", "4096", "-5", "big"]:
                response = await client.get(
                    "/sugarcubes/output-saving/preview",
                    params={**reference, "size": size},
                )
                assert response.status == 400
            response = await client.get(
                "/sugarcubes/output-saving/preview",
                params={**reference, "subfolder": "..", "size": "320"},
            )
            assert response.status == 400
            for invalid in [
                {"filename": "../wide.png"},
                {"filename": "C:\\wide.png"},
                {"filename": "wide.gif"},
                {"filename": "missing.png"},
                {"subfolder": "C:\\outside"},
                {"subfolder": "nested/../../outside"},
                {"type": "outside"},
            ]:
                response = await client.get(
                    "/sugarcubes/output-saving/preview",
                    params={**reference, **invalid, "size": "320"},
                )
                assert response.status == 400
        assert (output / "wide.png").read_bytes() == original
        assert sorted(path.name for path in output.iterdir()) == [
            "wide.jpg",
            "wide.png",
        ]

    asyncio.run(exercise())
