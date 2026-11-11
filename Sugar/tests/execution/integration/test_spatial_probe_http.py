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
"""Exercise bounded graph admission and explicit measurement over the real HTTP surface."""

from __future__ import annotations

import asyncio
from copy import deepcopy

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.execution.spatial_probe_graph import SpatialProbeGraph
from sugarcubes.execution.spatial_probe_http import install_spatial_probe_handlers
from sugarcubes.execution.spatial_probe_service import SpatialProbeService
from sugarcubes.execution.spatial_probe_values import (
    SpatialNodeContract,
    SpatialProbeExecution,
)


class Measurement:
    """Replace only external native scheduling and retain its received graph for assertions."""

    def __init__(self) -> None:
        """Start without accepted native work."""
        self.calls: list[SpatialProbeExecution] = []

    async def run(
        self, execution: SpatialProbeExecution
    ) -> tuple[tuple[int, int], tuple[int, int]]:
        """Measure a factor of 1.5 without introducing filesystem or image-output side effects."""
        self.calls.append(execution)
        return ((192, 96), (288, 192))


def contract(name: str) -> SpatialNodeContract:
    """Supply installed schemas while retaining production path discovery and experiment rewriting."""
    return {
        "EmptyImage": SpatialNodeContract({}, ("IMAGE",)),
        "SaveImage": SpatialNodeContract({"images": "IMAGE"}, (), output_node=True),
    }[name]


def test_probe_http_captures_measures_and_releases_without_saving() -> None:
    """Prove transport isolation, output selection, validation and idempotent handle release."""

    async def run() -> None:
        """Use an actual aiohttp server with a bounded native execution substitute."""
        app = web.Application()
        routes = web.RouteTableDef()
        measurement = Measurement()
        service = SpatialProbeService(SpatialProbeGraph(contract), measurement)
        install_spatial_probe_handlers(app, routes, service)
        install_spatial_probe_handlers(app, routes, service)
        app.add_routes(routes)
        prompt: ApiPrompt = {
            "1": {"class_type": "EmptyImage", "inputs": {"width": 1024, "height": 768}},
            "2": {
                "class_type": "SaveImage",
                "inputs": {"images": ["1", 0], "filename_prefix": "untouched"},
                "_meta": {"title": "<literal output>"},
            },
        }
        original = deepcopy(prompt)
        payload: dict[str, object] = {
            "prompt": prompt,
            "width_targets": [{"nodeId": "1", "inputName": "width"}],
            "height_targets": [{"nodeId": "1", "inputName": "height"}],
        }
        prefix = "/sugarcubes/v2/executions/spatial"
        async with TestClient(TestServer(app)) as client:
            response = await client.post(prefix + "/prepare", json=payload)
            assert (
                response.status == 200
                and response.headers["Cache-Control"] == "no-store"
            )
            value = await response.json()
            token = value["token"]
            assert value["outputs"] == [
                {"id": "2/images", "label": "<literal output> · 2"}
            ]
            assert measurement.calls == []
            assert (
                await client.post(
                    f"{prefix}/{token}/measure", json={"output": "missing"}
                )
            ).status == 422
            result = await client.post(
                f"{prefix}/{token}/measure", json={"output": "2/images"}
            )
            assert result.status == 200 and (await result.json())["scale"] == 1.5
            assert prompt == original and len(measurement.calls) == 1
            assert all(
                node["class_type"] != "SaveImage"
                for node in measurement.calls[0].prompt.values()
            )
            assert (await client.delete(f"{prefix}/{token}")).status == 204
            assert (await client.delete(f"{prefix}/{token}")).status == 204
            assert (
                await client.post(
                    f"{prefix}/{token}/measure", json={"output": "2/images"}
                )
            ).status == 422
            for invalid in [
                [],
                {},
                {**payload, "prompt": {}},
                {**payload, "width_targets": []},
                {
                    **payload,
                    "prompt": {
                        "x": {
                            "class_type": "SugarCubes.SpatialProbeMeasurement",
                            "inputs": {},
                        }
                    },
                },
            ]:
                assert (
                    await client.post(prefix + "/prepare", json=invalid)
                ).status == 422
            assert (
                await client.post(prefix + "/prepare", data=b'{"prompt":')
            ).status == 422
            assert len(measurement.calls) == 1

    asyncio.run(run())
