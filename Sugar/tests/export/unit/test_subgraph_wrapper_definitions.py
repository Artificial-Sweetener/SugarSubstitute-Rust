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
"""Prove native wrapper definitions retain exact nested field ownership."""

from __future__ import annotations

from typing import Any

import pytest

from sugarcubes.exporter.subgraph_wrapper_definitions import (
    build_subgraph_wrapper_definitions,
)

LEAF_ID = "11111111-1111-4111-8111-111111111111"
PARENT_ID = "22222222-2222-4222-8222-222222222222"


def test_nested_wrapper_inherits_the_exact_leaf_field_definition() -> None:
    """Resolve a public field recursively through another wrapper boundary."""

    wrappers = build_subgraph_wrapper_definitions(
        [_leaf_subgraph(), _parent_subgraph()],
        {
            "Sampler": {
                "input": {
                    "required": {
                        "steps": ["INT", {"default": 20, "min": 1, "max": 10000}]
                    }
                },
                "input_order": {"required": ["steps"]},
            }
        },
    )

    expected = ["INT", {"default": 20, "min": 1, "max": 10000}]
    assert wrappers[LEAF_ID]["input"]["required"]["steps"] == expected
    assert wrappers[PARENT_ID]["input"]["required"]["public_steps"] == expected


def test_wrapper_definition_fails_closed_without_its_exact_target_schema() -> None:
    """Reject an exposed widget whose portable meaning cannot be established."""

    with pytest.raises(ValueError, match="lacks the exact target definition 'Sampler'"):
        build_subgraph_wrapper_definitions([_leaf_subgraph()], {})


def _leaf_subgraph() -> dict[str, Any]:
    """Build the dependency wrapper around one concrete sampler field."""

    return _subgraph(
        wrapper_id=LEAF_ID,
        boundary_name="steps",
        node_type="Sampler",
        target_input="steps",
        link_id=10,
    )


def _parent_subgraph() -> dict[str, Any]:
    """Build a parent wrapper that exposes the leaf wrapper's field."""

    return _subgraph(
        wrapper_id=PARENT_ID,
        boundary_name="public_steps",
        node_type=LEAF_ID,
        target_input="steps",
        link_id=20,
    )


def _subgraph(
    *,
    wrapper_id: str,
    boundary_name: str,
    node_type: str,
    target_input: str,
    link_id: int,
) -> dict[str, Any]:
    """Build one single-widget native subgraph definition."""

    return {
        "id": wrapper_id,
        "name": wrapper_id,
        "inputs": [
            {
                "id": f"{wrapper_id}-input",
                "name": boundary_name,
                "label": boundary_name,
                "type": "INT",
                "linkIds": [link_id],
            }
        ],
        "outputs": [],
        "nodes": [
            {
                "id": 1,
                "type": node_type,
                "inputs": [
                    {
                        "name": target_input,
                        "type": "INT",
                        "link": link_id,
                        "widget": {"name": target_input},
                    }
                ],
                "widgets_values": [20],
            }
        ],
        "links": [
            {
                "id": link_id,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 1,
                "target_slot": 0,
                "type": "INT",
            }
        ],
    }
