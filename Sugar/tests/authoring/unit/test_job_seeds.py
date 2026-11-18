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
"""Verify which seeds are read from one job's Comfy history record."""

from __future__ import annotations

import asyncio
import json

from sugarcubes.authoring.job_seeds import JobSeed, read_job_seeds
from sugarcubes.backend.job_seed_routes import read_job_seeds_route
from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    decode_json_response,
)

_WHOLE_SEED = 12345678901234567890


def _record() -> dict[str, object]:
    """Build the record Comfy keeps for a job of two named Cubes and one plain node."""

    def cube(instance: str, alias: str) -> dict[str, object]:
        return {
            "properties": {
                "sugarcubes_cube": {"instance_id": instance, "instance_alias": alias}
            }
        }

    nodes = {
        "run-1:ksampler:ksampler": {
            "class_type": "KSampler",
            "inputs": {"seed": _WHOLE_SEED, "steps": 30, "model": ["run-1:models", 0]},
        },
        "run-1:variation": {"inputs": {"variation_seed": 9, "noise_seed": 4}},
        "run-10:ksampler": {"inputs": {"seed": 7, "enabled": True}},
        "run-1:linked": {"inputs": {"seed": ["run-1:variation", 0]}},
        "17": {"inputs": {"seed": 3}},
    }
    workflow = {
        "nodes": [cube("run-1", "Text to Image"), cube("run-10", "Upscale"), {}]
    }
    return {"prompt": [4, "job", nodes, {"extra_pnginfo": {"workflow": workflow}}, []]}


def test_each_named_cube_gives_the_seeds_its_nodes_ran_with() -> None:
    """Read whole-number seed inputs of Cube nodes, by the Cube's name, with every digit."""

    expected = (
        JobSeed("Text to Image", "ksampler", "seed", str(_WHOLE_SEED)),
        JobSeed("Text to Image", "variation", "noise_seed", "4"),
        JobSeed("Upscale", "ksampler", "seed", "7"),
    )

    assert read_job_seeds(_record()) == expected
    assert read_job_seeds({"job": _record()}) == expected


def test_a_record_without_cubes_or_without_a_prompt_gives_no_seeds() -> None:
    """Refuse nothing and invent nothing for records this reader cannot use."""

    assert read_job_seeds(None) == ()
    assert read_job_seeds({"job": {"outputs": {}}}) == ()
    assert read_job_seeds({"prompt": [1, "job", {"17": {"inputs": {"seed": 3}}}]}) == ()
    record = _record()
    assert read_job_seeds({"prompt": [*record["prompt"][:3], {}, []]}) == ()  # type: ignore[index]


def test_the_route_reads_the_reply_text_so_a_large_seed_keeps_its_digits() -> None:
    """Return digits as text, and refuse a request without the reply's text."""

    response = asyncio.run(
        read_job_seeds_route(
            FakeRequest(body={"history_text": json.dumps({"job": _record()})})
        )
    )

    assert response.status == 200
    body = decode_json_response(response)
    assert isinstance(body, dict)
    assert body["seeds"][0] == {
        "alias": "Text to Image",
        "node_symbol": "ksampler",
        "input_name": "seed",
        "digits": str(_WHOLE_SEED),
    }
    for bad in ({}, {"history_text": 3}, {"history_text": "{"}):
        assert asyncio.run(read_job_seeds_route(FakeRequest(body=bad))).status == 400
