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
"""Exercise production artifact route composition over real isolated same-origin HTTP."""

import asyncio
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
from PIL import Image

from sugarcubes.output_saving.infrastructure.http_routes import register_saving_routes
from sugarcubes.output_saving.infrastructure.runtime import OutputSavingRuntime


def test_composed_receipt_resolver_and_closed_origin_boundary(tmp_path: Path) -> None:
    """Confirm exact ordinary and temporary files, moved files and per-item overwritten refusal."""

    async def exercise() -> None:
        """Run the real HTTP codec, service and file reader without loading Comfy or inference."""
        output = tmp_path / "output"
        output.mkdir()
        temp = tmp_path / "temp"
        temp.mkdir()
        path = output / "final.png"
        with Image.new("RGB", (4, 3), "red") as image:
            image.save(path)
            image.save(temp / "temporary.png")
        runtime = OutputSavingRuntime(
            state_root=tmp_path / "state", output_root=output, temporary_root=temp
        )
        routes = web.RouteTableDef()
        register_saving_routes(routes, runtime)
        app = web.Application()
        app.add_routes(routes)

        async def shutdown(application: web.Application) -> None:
            """Observe production worker shutdown before releasing the isolated HTTP host."""
            await runtime.artifact_workers.close()

        app.on_shutdown.append(shutdown)
        async with TestClient(TestServer(app)) as client:
            origin = str(client.make_url("/")).rstrip("/")
            headers = {"Origin": origin}
            file = {"filename": path.name, "subfolder": "", "type": "output"}
            for invalid in [
                {},
                {"Origin": "https://foreign.example"},
                {**headers, "X-Forwarded-For": "127.0.0.1"},
                {**headers, "Sec-Fetch-Site": "cross-site"},
            ]:
                response = await client.post(
                    "/sugarcubes/output-saving/artifact-receipt",
                    json={"file": file},
                    headers=invalid,
                )
                assert response.status == 403
            response = await client.post(
                "/sugarcubes/output-saving/artifact-receipt",
                json={"file": file},
                headers=headers,
            )
            assert response.status == 200
            assert response.headers["Cache-Control"] == "no-store"
            value = await response.json()
            assert value["available"] is True
            receipt = value["receipt"]
            assert receipt["file"] == {
                "storage": "output",
                "filename": path.name,
                "subfolder": "",
                "managedArtifact": None,
            }
            assert (
                receipt["width"],
                receipt["height"],
                receipt["metadataEmbedded"],
            ) == (4, 3, False)
            expected = {
                key: value for key, value in receipt.items() if key != "animated"
            }
            expected["promptId"] = None
            moved = output / "renamed.png"
            path.rename(moved)
            response = await client.post(
                "/sugarcubes/output-saving/artifact-resolve",
                json={"expected": expected},
                headers=headers,
            )
            resolved = await response.json()
            assert resolved["available"] is True
            assert resolved["receipt"]["file"]["filename"] == "renamed.png"
            moved.rename(path)
            with Image.new("RGB", (4, 3), "blue") as image:
                image.save(path)
            response = await client.post(
                "/sugarcubes/output-saving/artifact-resolve",
                json={"expected": expected},
                headers=headers,
            )
            assert (await response.json())["code"] == "changed"
            response = await client.post(
                "/sugarcubes/output-saving/artifact-receipt",
                json={
                    "file": {
                        "filename": "temporary.png",
                        "subfolder": "",
                        "type": "temp",
                    }
                },
                headers=headers,
            )
            temporary = await response.json()
            assert temporary["available"] is True
            assert temporary["receipt"]["file"]["storage"] == "temp"
            requests: list[dict[str, object]] = [
                {"file": {**file, "subfolder": "../outside"}},
                {"file": {**file, "filename": "CON.png"}},
                {"file": file, "root": str(output)},
                {"expected": {**expected, "digest": "A" * 64}},
                {"expected": {**expected, "bytes": True}},
                {
                    "expected": {
                        **expected,
                        "file": {**expected["file"], "managedArtifact": "unregistered"},
                    }
                },
            ]
            for request in requests:
                route = (
                    "artifact-resolve" if "expected" in request else "artifact-receipt"
                )
                response = await client.post(
                    "/sugarcubes/output-saving/" + route, json=request, headers=headers
                )
                assert (await response.json())["code"] == "invalid_reference"
            response = await client.post(
                "/sugarcubes/output-saving/artifact-receipt",
                data='{"file":"' + "x" * 17000 + '"}',
                headers={**headers, "Content-Type": "application/json"},
            )
            assert (await response.json())["code"] == "invalid_reference"
        assert path.is_file()
        assert (temp / "temporary.png").is_file()

    asyncio.run(exercise())
