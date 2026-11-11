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
"""Protect persisted widget snapshots that delegate values to graph boundaries."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sugarcubes.cube_model.widget_values import canonicalize_subgraph_widget_values
from sugarcubes.cube_model.widget_values import decode_versioned_widget_snapshot
from sugarcubes.cube_model.widget_values import decode_workflow_widget_snapshot
from sugarcubes.cube_model.native_subgraph_defaults import (
    native_boundary_widget_names,
    native_subgraph_defaults,
    native_widget_defaults,
)


def test_boundary_linked_widget_without_snapshot_value_remains_loadable() -> None:
    """Accept a proxy widget whose runtime value is owned by the subgraph boundary."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "sampler",
            "inputs": [{"name": "sampler_name", "linkIds": [267], "type": "COMBO"}],
            "links": [
                {
                    "id": 267,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 155,
                    "target_slot": 0,
                    "type": "COMBO",
                }
            ],
            "nodes": [
                {
                    "id": 155,
                    "type": "KSamplerSelect",
                    "inputs": [
                        {
                            "name": "sampler_name",
                            "type": "COMBO",
                            "widget": {"name": "sampler_name"},
                            "link": 267,
                        }
                    ],
                    "widgets_values": [],
                }
            ],
        }
    ]
    definitions = {
        "KSamplerSelect": {
            "input": {"required": {"sampler_name": ["LIST"]}},
            "input_order": {"required": ["sampler_name"]},
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [None]


def test_native_subgraph_portable_null_projects_current_picker_default() -> None:
    """Treat an unset local resource as node-owned when preparing Cube display."""

    subgraph = {
        "id": "upscale",
        "inputs": [{"name": "model_name", "linkIds": [1], "type": "COMBO"}],
        "nodes": [
            {
                "id": 10,
                "type": "UpscaleModelLoader",
                "inputs": [
                    {
                        "name": "model_name",
                        "link": 1,
                        "widget": {"name": "model_name"},
                    }
                ],
                "widgets_values": [None],
            }
        ],
        "links": [
            {
                "id": 1,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 10,
                "target_slot": 0,
                "type": "COMBO",
            }
        ],
    }
    definitions = {
        "UpscaleModelLoader": {
            "input": {"required": {"model_name": ["COMBO"]}},
            "input_order": {"required": ["model_name"]},
        }
    }
    live_definition = {
        "input": {
            "required": {"model_name": [["1x-local-default.pth", "4x-other.pth"]]}
        },
        "input_order": {"required": ["model_name"]},
    }

    assert native_subgraph_defaults(
        subgraph,
        definitions,
        resolve_live_definition=lambda class_type: (
            live_definition if class_type == "UpscaleModelLoader" else None
        ),
    ) == {"model_name": "1x-local-default.pth"}


def test_unlinked_widget_without_snapshot_value_still_fails_closed() -> None:
    """Keep rejecting missing values when no graph boundary owns the widget."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "sampler",
            "nodes": [
                {
                    "id": 155,
                    "type": "KSamplerSelect",
                    "inputs": [
                        {
                            "name": "sampler_name",
                            "type": "COMBO",
                            "widget": {"name": "sampler_name"},
                            "link": None,
                        }
                    ],
                    "widgets_values": [],
                }
            ],
        }
    ]
    definitions = {
        "KSamplerSelect": {
            "input": {"required": {"sampler_name": ["LIST"]}},
            "input_order": {"required": ["sampler_name"]},
        }
    }

    try:
        canonicalize_subgraph_widget_values(subgraphs, definitions)
    except ValueError as exc:
        assert "missing the value for widget 'sampler_name'" in str(exc)
    else:
        raise AssertionError("Unlinked missing widget value was accepted")


def test_compact_snapshot_maps_values_only_to_non_boundary_widgets() -> None:
    """Decode an unambiguous compact snapshot after boundary defaults were omitted."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "sampler",
            "links": [
                {
                    "id": link_id,
                    "origin_id": -10,
                    "origin_slot": index,
                    "target_id": 156,
                    "target_slot": index,
                    "type": "INT",
                }
                for index, link_id in enumerate((211, 213))
            ],
            "nodes": [
                {
                    "id": 156,
                    "type": "EmptyFlux2LatentImage",
                    "inputs": [
                        {"name": "width", "link": 211, "widget": {"name": "width"}},
                        {"name": "height", "link": 213, "widget": {"name": "height"}},
                        {
                            "name": "batch_size",
                            "link": None,
                            "widget": {"name": "batch_size"},
                        },
                    ],
                    "widgets_values": [1],
                }
            ],
        }
    ]
    definitions = {
        "EmptyFlux2LatentImage": {
            "input": {
                "required": {
                    "width": ["INT"],
                    "height": ["INT"],
                    "batch_size": ["INT"],
                }
            },
            "input_order": {"required": ["width", "height", "batch_size"]},
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [None, None, 1]


def test_native_lowering_recovers_widget_order_from_embedded_cube_definition() -> None:
    """Execute legacy native nodes whose Cube snapshot retained its own definition."""

    node = {
        "id": "checkpoint",
        "type": "SimpleLoadCheckpoint",
        "inputs": [],
        "widgets_values": ["SDXL/model.safetensors", "Use Checkpoint VAE", False],
    }
    definition = {
        "input": {
            "required": {
                "ckpt_name": ["COMBO"],
                "vae_name": ["COMBO"],
                "clip_skip": ["BOOLEAN"],
            }
        },
        "input_order": {
            "required": ["ckpt_name", "vae_name", "clip_skip"],
        },
    }

    assert native_widget_defaults(node, definition, frozenset()) == {
        "ckpt_name": "SDXL/model.safetensors",
        "vae_name": "Use Checkpoint VAE",
        "clip_skip": False,
    }


def test_linked_widget_values_use_saved_input_identities_not_definition_order() -> None:
    """Keep a later node-definition reorder from shifting a saved full snapshot."""

    node = {
        "inputs": [
            {"name": "width", "link": 11, "widget": {"name": "width"}},
            {"name": "height", "link": 12, "widget": {"name": "height"}},
            {"name": "batch_size", "link": None, "widget": {"name": "batch_size"}},
        ],
        "widgets_values": [1080, 1512, 2],
    }
    reordered_definition = {
        "input": {
            "required": {
                "batch_size": ["INT"],
                "height": ["INT"],
                "width": ["INT"],
            }
        },
        "input_order": {"required": ["batch_size", "height", "width"]},
    }

    snapshot = decode_workflow_widget_snapshot(node, reordered_definition)

    assert snapshot is not None
    assert snapshot.source == "serialized_inputs_with_link_anchors"
    assert snapshot.values == {"batch_size": 2}


def test_exact_version_excludes_linked_widgets_missing_from_anonymous_array() -> None:
    """Recover only the unlinked historical field from the Anima prompt node."""

    node = {
        "inputs": [
            {
                "name": "positive_prompt",
                "link": 5,
                "widget": {"name": "positive_prompt"},
            },
            {
                "name": "negative_prompt",
                "link": 6,
                "widget": {"name": "negative_prompt"},
            },
        ],
        "widgets_values": ["prompt-control style"],
    }
    historical_definition = {
        "input": {
            "required": {
                "encode_style": ["STRING", {"forceInput": True}],
                "positive_prompt": ["STRING"],
                "negative_prompt": ["STRING"],
            }
        },
        "input_order": {
            "required": ["encode_style", "positive_prompt", "negative_prompt"]
        },
    }

    snapshot = decode_versioned_widget_snapshot(
        node,
        historical_definition,
        versioned_widget_names=[
            "encode_style",
            "positive_prompt",
            "negative_prompt",
        ],
    )

    assert snapshot is not None
    assert snapshot.source == "exact_versioned_definition"
    assert snapshot.values == {"encode_style": "prompt-control style"}


def test_boundary_link_identity_survives_string_normalization() -> None:
    """Recognize Comfy integer input links in a normalized string-key index."""

    node = {"inputs": [{"name": "seed", "link": 1680, "widget": {"name": "seed"}}]}
    links: Mapping[object, Mapping[str, object]] = {
        "1680": {
            "id": 1680,
            "origin_id": -10,
            "target_id": 1886,
            "target_slot": 4,
        }
    }

    assert native_boundary_widget_names(node, links) == frozenset({"seed"})
