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
"""Verify normal prompt input preparation uses the shared projector and validates immutable uploads."""

from __future__ import annotations

import asyncio
from pathlib import Path
from types import SimpleNamespace

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.execution.prepared_input_values import PreparedInputAsset
from sugarcubes.input_canvas.infrastructure.prepared_input_http import (
    install_prepared_input_route,
)


def test_prepared_input_http_validates_upload_and_delegates_once(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Exercise transport success, missing files and malformed requests without queueing the graph."""
    prepared_file = "sugarcubes/sources/source-00000000-0000-0000-0000-000000000001.png"
    calls: list[tuple[ApiPrompt, tuple[PreparedInputAsset, ...]]] = []

    def validate(file: str) -> Path:
        """Model the external Comfy upload filesystem boundary."""
        assert file == prepared_file
        return tmp_path

    def prepare(prompt: ApiPrompt, assets: tuple[PreparedInputAsset, ...]) -> ApiPrompt:
        """Retain the exact typed inputs supplied to the execution owner."""
        calls.append((prompt, assets))
        return {
            **prompt,
            "prepared": {
                "class_type": "LoadImageMask",
                "inputs": {"image": assets[0].prepared_file, "channel": "alpha"},
            },
        }

    monkeypatch.setattr(
        "sugarcubes.input_canvas.infrastructure.prepared_input_http.validate_input_file",
        validate,
    )

    async def run() -> None:
        """Use the registered aiohttp surface, including idempotent installation."""
        app = web.Application()
        routes = web.RouteTableDef()
        server = SimpleNamespace(app=app, routes=routes)
        install_prepared_input_route(server, prepare)
        install_prepared_input_route(server, prepare)
        app.add_routes(routes)
        payload: dict[str, object] = {
            "prompt": {
                "mask": {
                    "class_type": "LoadImageMask",
                    "inputs": {"image": "original.png"},
                }
            },
            "assets": [
                {
                    "nodePath": "mask",
                    "classType": "LoadImageMask",
                    "field": "image",
                    "file": "original.png",
                    "slot": 0,
                    "kind": "mask",
                    "preparedFile": prepared_file,
                }
            ],
        }
        async with TestClient(TestServer(app)) as client:
            response = await client.post(
                "/sugarcubes/v2/canvas-inputs/prepare", json=payload
            )
            assert (
                response.status == 200
                and response.headers["Cache-Control"] == "no-store"
            )
            value = await response.json()
            assert value["prompt"]["prepared"]["inputs"]["image"] == prepared_file
            for invalid in [
                [],
                {**payload, "prompt": []},
                {**payload, "assets": [{"slot": 0}]},
                {**payload, "assets": "invalid"},
            ]:
                failed = await client.post(
                    "/sugarcubes/v2/canvas-inputs/prepare", json=invalid
                )
                assert failed.status == 400
            assert len(calls) == 1
            assert calls[0][1][0].prepared_file == prepared_file

    asyncio.run(run())
