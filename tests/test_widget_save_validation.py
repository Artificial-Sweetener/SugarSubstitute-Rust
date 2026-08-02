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
"""Validate stable widget relationships without relying on current order."""

from __future__ import annotations

from typing import Any

import pytest

from sugarcubes.cube_model.authored_default_policy import (
    sanitize_authored_defaults_payload,
)
from sugarcubes.exporter.value_validation import (
    PersistedValueError,
    validate_named_node_inputs,
)
from sugarcubes.cube_model.widget_values import (
    WidgetSnapshotError,
    canonicalize_subgraph_widget_values,
    decode_workflow_widget_snapshot,
)


def test_explicit_snapshot_preserves_names_across_widget_reordering() -> None:
    """Name-addressed values remain stable regardless of mapping iteration order."""

    snapshot = decode_workflow_widget_snapshot(
        {
            "sugarcubes_widget_values": {
                "method": "dpmpp",
                "steps": 30,
            }
        },
        _definition(),
    )

    assert snapshot is not None
    assert snapshot.values == {"method": "dpmpp", "steps": 30}


def test_same_snapshot_names_decode_positional_values_without_live_order() -> None:
    """Stored input identities decode values from their contemporaneous array."""

    snapshot = decode_workflow_widget_snapshot(
        {
            "inputs": [
                {"name": "method", "widget": {"name": "method"}},
                {"name": "steps", "widget": {"name": "steps"}},
            ],
            "widgets_values": ["dpmpp", 30],
        },
        _definition(),
    )

    assert snapshot is not None
    assert snapshot.values == {"method": "dpmpp", "steps": 30}


def test_linked_widget_inputs_do_not_require_positional_values() -> None:
    """Connected subgraph widgets receive values from links, not widget arrays."""

    snapshot = decode_workflow_widget_snapshot(
        {
            "inputs": [
                {
                    "name": "sampler_name",
                    "link": 272,
                    "widget": {"name": "sampler_name"},
                },
                {"name": "width", "link": 273, "widget": {"name": "width"}},
                {"name": "steps", "link": 275, "widget": {"name": "steps"}},
            ],
            "widgets_values": [],
        },
        _definition(),
    )

    assert snapshot is not None
    assert snapshot.values == {}


def test_linked_widget_inputs_preserve_unlinked_definition_values() -> None:
    """Definition order recovers only values not supplied by graph links."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "sampler",
            "nodes": [
                {
                    "id": 156,
                    "type": "EmptyFlux2LatentImage",
                    "inputs": [
                        {"name": "width", "link": 211, "widget": {"name": "width"}},
                        {
                            "name": "height",
                            "link": 213,
                            "widget": {"name": "height"},
                        },
                        {
                            "name": "batch_size",
                            "type": "INT",
                            "widget": {"name": "batch_size"},
                        },
                    ],
                    "widgets_values": [1024, 1024, 1],
                }
            ],
        }
    ]
    definitions = {
        "EmptyFlux2LatentImage": {
            "input": {
                "required": {
                    "width": ["INT", {"default": 1024}],
                    "height": ["INT", {"default": 1024}],
                    "batch_size": ["INT", {"default": 1}],
                }
            },
            "input_order": {"required": ["width", "height", "batch_size"]},
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)
    node = canonical[0]["nodes"][0]

    assert node["widgets_values"] == [1]
    assert node["inputs"][-1] == {
        "name": "batch_size",
        "type": "INT",
        "widget": {"name": "batch_size"},
    }


def test_linked_widget_inputs_ignore_stale_positional_values() -> None:
    """Connected widgets may retain stale UI values that do not drive execution."""

    snapshot = decode_workflow_widget_snapshot(
        {
            "inputs": [
                {
                    "name": "sampler_name",
                    "link": 272,
                    "widget": {"name": "sampler_name"},
                }
            ],
            "widgets_values": ["euler"],
        },
        {
            "input": {
                "required": {"sampler_name": [["euler", "dpmpp"], {"default": "euler"}]}
            },
            "input_order": {"required": ["sampler_name"]},
        },
    )

    assert snapshot is not None
    assert snapshot.values == {}


def test_explicit_subgraph_snapshot_adds_persisted_widget_identities() -> None:
    """Live name maps make imported Comfy subgraph widgets safe to persist."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "subgraph",
            "nodes": [
                {
                    "id": 125,
                    "type": "UNETLoader",
                    "inputs": [],
                    "widgets_values": ["stale.safetensors", "default"],
                    "sugarcubes_widget_values": {
                        "unet_name": "model.safetensors",
                        "weight_dtype": "default",
                    },
                }
            ],
        }
    ]
    definitions = {
        "UNETLoader": {
            "input": {
                "required": {
                    "unet_name": [["model.safetensors"], {}],
                    "weight_dtype": [["default"], {}],
                }
            }
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)
    node = canonical[0]["nodes"][0]

    assert node["inputs"] == [
        {
            "name": "unet_name",
            "type": ["model.safetensors"],
            "widget": {"name": "unet_name"},
        },
        {
            "name": "weight_dtype",
            "type": ["default"],
            "widget": {"name": "weight_dtype"},
        },
    ]
    assert node["widgets_values"] == [None, "default"]
    assert "sugarcubes_widget_values" not in node


def test_ambiguous_positional_values_fail_closed() -> None:
    """Values without same-snapshot names cannot shift into current fields."""

    with pytest.raises(WidgetSnapshotError, match="no same-snapshot"):
        decode_workflow_widget_snapshot(
            {"widgets_values": [30, "dpmpp"]},
            _definition(),
        )


def test_portable_value_validation_rejects_named_range_corruption() -> None:
    """Portable values remain constrained by their same-named live fields."""

    definition = {
        "input": {
            "required": {
                "white_point": [
                    "FLOAT",
                    {"default": 0.99, "min": 0.02, "max": 1.0},
                ]
            }
        }
    }

    with pytest.raises(PersistedValueError, match="white_point.*above maximum 1.0"):
        validate_named_node_inputs(
            node_id=10,
            class_type="Example.Node",
            inputs={"white_point": 2048},
            definition=definition,
        )


def test_portable_picker_validation_rejects_invalid_stable_choice() -> None:
    """Stable enum values remain validated after positional checks are removed."""

    with pytest.raises(PersistedValueError, match="method.*stable choice"):
        validate_named_node_inputs(
            node_id=10,
            class_type="Example.Node",
            inputs={"method": "invalid"},
            definition=_definition(),
        )


def test_machine_local_picker_inventory_does_not_gate_cube_saves() -> None:
    """Checkpoint inventory membership is outside portable authored validation."""

    definition = {
        "input": {
            "required": {
                "ckpt_name": [["machine-a.safetensors"], {}],
            }
        }
    }

    validate_named_node_inputs(
        node_id=10,
        class_type="CheckpointLoaderSimple",
        inputs={"ckpt_name": ""},
        definition=definition,
    )


def test_subgraph_persistence_removes_local_and_volatile_values_by_name() -> None:
    """Subgraph arrays retain shape without shipping machine or seed values."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "subgraph",
            "nodes": [
                {
                    "id": 7,
                    "type": "SimpleSyrup.SimpleLoadCheckpoint",
                    "inputs": [
                        {"name": "ckpt_name", "widget": {"name": "ckpt_name"}},
                        {"name": "vae_name", "widget": {"name": "vae_name"}},
                        {"name": "clip_skip", "widget": {"name": "clip_skip"}},
                    ],
                    "widgets_values": [
                        "machine-a.safetensors",
                        "machine-a.vae.safetensors",
                        False,
                    ],
                },
                {
                    "id": 8,
                    "type": "KSampler",
                    "inputs": [
                        {"name": "seed", "widget": {"name": "seed"}},
                        {"name": "steps", "widget": {"name": "steps"}},
                    ],
                    "widgets_values": [1234, "randomize", 30],
                },
            ],
        }
    ]
    definitions = {
        "SimpleSyrup.SimpleLoadCheckpoint": {
            "input": {
                "required": {
                    "ckpt_name": [["machine-a.safetensors"], {}],
                    "vae_name": [["machine-a.vae.safetensors"], {}],
                    "clip_skip": ["BOOLEAN", {"default": False}],
                }
            }
        },
        "KSampler": {
            "input": {
                "required": {
                    "seed": ["INT", {"control_after_generate": True}],
                    "steps": ["INT", {"default": 20}],
                }
            }
        },
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [None, None, False]
    assert canonical[0]["nodes"][1]["widgets_values"] == [None, 30]
    assert subgraphs[0]["nodes"][0]["widgets_values"][0] == "machine-a.safetensors"


def test_portability_policy_removes_scalar_resources_but_preserves_connections() -> (
    None
):
    """Resource sockets remain connectable while local scalar choices stay local."""

    payload: dict[str, Any] = {
        "implementation": {
            "nodes": {
                "scalar": {
                    "class_type": "CheckpointLoaderSimple",
                    "inputs": {"ckpt_name": "machine-a.safetensors"},
                },
                "connected": {
                    "class_type": "CheckpointLoaderSimple",
                    "inputs": {"ckpt_name": ["resource_name", 0]},
                },
            }
        },
        "surface": {"controls": []},
        "flavors": {"authored": []},
    }

    sanitize_authored_defaults_payload(payload)

    nodes = payload["implementation"]["nodes"]
    assert nodes["scalar"]["inputs"] == {}
    assert nodes["connected"]["inputs"] == {"ckpt_name": ["resource_name", 0]}


def _definition() -> dict[str, object]:
    """Build a stable enum and scalar definition fixture."""

    return {
        "input": {
            "required": {
                "steps": ["INT", {"default": 20, "min": 1, "max": 100}],
                "method": [["euler", "dpmpp"], {"default": "euler"}],
            }
        },
        "input_order": {"required": ["steps", "method"]},
    }
