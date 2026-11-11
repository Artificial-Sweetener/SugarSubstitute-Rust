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
    validate_subgraph_widget_values,
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


def test_subgraph_boundary_widget_inputs_preserve_authored_defaults() -> None:
    """Keep boundary defaults while discarding ordinary internal-link anchors."""

    subgraphs: list[dict[str, Any]] = [
        {
            "id": "sampler",
            "links": [
                {
                    "id": 41,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 7,
                    "target_slot": 0,
                    "type": "INT",
                },
                {
                    "id": 42,
                    "origin_id": 6,
                    "origin_slot": 0,
                    "target_id": 7,
                    "target_slot": 1,
                    "type": "INT",
                },
            ],
            "nodes": [
                {
                    "id": 7,
                    "type": "Example.Node",
                    "inputs": [
                        {"name": "width", "link": 41, "widget": {"name": "width"}},
                        {
                            "name": "internal_height",
                            "link": 42,
                            "widget": {"name": "internal_height"},
                        },
                        {"name": "steps", "widget": {"name": "steps"}},
                    ],
                    "widgets_values": [1080, 777, 30],
                }
            ],
        }
    ]
    definitions = {
        "Example.Node": {
            "input": {
                "required": {
                    "width": ["INT", {"default": 512}],
                    "internal_height": ["INT", {"default": 512}],
                    "steps": ["INT", {"default": 20}],
                }
            },
            "input_order": {"required": ["width", "internal_height", "steps"]},
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [1080, 30]


def test_canonical_subgraph_boundary_values_pass_save_validation() -> None:
    """Validate canonical values with the same boundary context used to build them."""

    widget_names = [
        "width",
        "height",
        "resize_mode",
        "sampling",
        "processor",
        "divisible_by",
        "crop_position",
        "pad_color",
        "max_batch_size",
        "sinc_window",
        "precision",
    ]
    widget_values = [
        1024,
        1024,
        "Keep AR",
        "lanczos",
        "gpu",
        2,
        "center",
        "0, 0, 0",
        0,
        3,
        "fp32",
    ]
    inputs: list[dict[str, Any]] = [
        {"name": name, "widget": {"name": name}} for name in widget_names
    ]
    inputs[0]["link"] = 501
    inputs[1]["link"] = 502
    inputs[3]["link"] = 503
    subgraphs: list[dict[str, Any]] = [
        {
            "id": "upscale-by-factor",
            "links": [
                {
                    "id": 501,
                    "origin_id": 2159,
                    "origin_slot": 0,
                    "target_id": 2160,
                    "target_slot": 0,
                    "type": "INT",
                },
                {
                    "id": 502,
                    "origin_id": 2159,
                    "origin_slot": 1,
                    "target_id": 2160,
                    "target_slot": 1,
                    "type": "INT",
                },
                {
                    "id": 503,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 2160,
                    "target_slot": 3,
                    "type": "COMBO",
                },
            ],
            "nodes": [
                {
                    "id": 2160,
                    "type": "SimpleSyrup.ResizeImageToTarget",
                    "inputs": inputs,
                    "widgets_values": widget_values,
                    "sugarcubes_widget_values": dict(zip(widget_names, widget_values)),
                }
            ],
        }
    ]
    definitions = {
        "SimpleSyrup.ResizeImageToTarget": {
            "input": {
                "required": {
                    "width": ["INT", {"default": 512}],
                    "height": ["INT", {"default": 512}],
                    "resize_mode": [["Keep AR", "Crop"], {"default": "Keep AR"}],
                    "sampling": [["lanczos", "nearest"], {"default": "lanczos"}],
                    "processor": [["gpu", "cpu"], {"default": "gpu"}],
                    "divisible_by": ["INT", {"default": 1}],
                    "crop_position": [["center", "top"], {"default": "center"}],
                    "pad_color": ["STRING", {"default": "0, 0, 0"}],
                    "max_batch_size": ["INT", {"default": 0}],
                    "sinc_window": ["INT", {"default": 3}],
                    "precision": [["fp32", "fp16"], {"default": "fp32"}],
                }
            },
            "input_order": {"required": widget_names},
        }
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [
        "Keep AR",
        "lanczos",
        "gpu",
        2,
        "center",
        "0, 0, 0",
        0,
        3,
        "fp32",
    ]
    validate_subgraph_widget_values(canonical, definitions)


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


def test_file_inventory_picker_is_machine_local_without_known_node_names() -> None:
    """Infer arbitrary host file inventories from picker options, not node allowlists."""

    definition = {
        "input": {
            "required": {
                "asset": [
                    "COMBO",
                    {
                        "options": [
                            "models\\producer-only.safetensors",
                            "models\\another.ckpt",
                        ]
                    },
                ]
            }
        }
    }

    validate_named_node_inputs(
        node_id="arbitrary-node",
        class_type="ThirdParty.ArbitraryLoader",
        inputs={"asset": ""},
        definition=definition,
    )


def test_resource_shaped_picker_names_are_machine_local_without_inventory() -> None:
    """Keep empty loader inventories portable through conventional resource names."""

    definition = {
        "input": {
            "required": {
                "diffusion_model": ["COMBO", {"options": []}],
            }
        }
    }

    validate_named_node_inputs(
        node_id="anima-loader",
        class_type="ThirdParty.Loader",
        inputs={"diffusion_model": ""},
        definition=definition,
    )


def test_unselected_stable_picker_does_not_block_or_persist() -> None:
    """Treat an empty unavailable picker choice as an unset authored value."""

    definition = {
        "input": {
            "required": {
                "precision": ["COMBO", {"options": ["default", "fp16"]}],
            }
        }
    }
    subgraphs: list[dict[str, Any]] = [
        {
            "id": "picker",
            "nodes": [
                {
                    "id": 7,
                    "type": "ThirdParty.Picker",
                    "inputs": [{"name": "precision", "widget": {"name": "precision"}}],
                    "widgets_values": [""],
                }
            ],
        }
    ]

    canonical = canonicalize_subgraph_widget_values(
        subgraphs,
        {"ThirdParty.Picker": definition},
    )

    assert canonical[0]["nodes"][0]["widgets_values"] == [None]
    validate_subgraph_widget_values(canonical, {"ThirdParty.Picker": definition})


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
                {
                    "id": 9,
                    "type": "Example.FileBatch",
                    "inputs": [
                        {"name": "files", "widget": {"name": "files"}},
                        {"name": "mode", "widget": {"name": "mode"}},
                    ],
                    "widgets_values": [["a.png", "b.png"], "alpha"],
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
        "Example.FileBatch": {
            "input": {
                "required": {
                    "files": [
                        "COMBO",
                        {
                            "file_upload": True,
                            "default": [],
                            "multiselect": True,
                            "options": ["a.png", "b.png"],
                        },
                    ],
                    "mode": [["alpha", "red"], {"default": "alpha"}],
                }
            }
        },
    }

    canonical = canonicalize_subgraph_widget_values(subgraphs, definitions)

    assert canonical[0]["nodes"][0]["widgets_values"] == [None, None, False]
    assert canonical[0]["nodes"][1]["widgets_values"] == [None, 30]
    assert canonical[0]["nodes"][2]["widgets_values"] == [None, "alpha"]
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
