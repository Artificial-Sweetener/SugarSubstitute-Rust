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
"""Characterize stable widget identity across real Comfy v3 save payloads."""

from __future__ import annotations

from typing import Any

from sugarcubes.exporter import export_cubes

CHECKPOINT_CLASS = "SimpleSyrup.SimpleLoadCheckpoint"
PROMPT_CLASS = "SimpleSyrup.ScheduleAndEncodePromptsWithPromptControl"


def test_export_accepts_unselected_machine_local_resource_widgets() -> None:
    """Machine-local resource selectors do not block or seed authored values."""

    cube = _export_single_node(
        class_type=CHECKPOINT_CLASS,
        prompt_inputs={"ckpt_name": "", "vae_name": "", "clip_skip": False},
        workflow_inputs=[],
        widget_values=["", "", False],
    )

    controls = _controls_by_input(cube)
    authored_values = cube["flavors"]["authored"][0]["values"]
    node_inputs = next(iter(cube["implementation"]["nodes"].values()))["inputs"]

    assert {"ckpt_name", "vae_name", "clip_skip"} <= controls.keys()
    assert "ckpt_name" not in node_inputs
    assert "vae_name" not in node_inputs
    assert node_inputs.get("clip_skip", False) is False
    assert controls["ckpt_name"]["control_id"] not in authored_values
    assert controls["vae_name"]["control_id"] not in authored_values
    assert authored_values[controls["clip_skip"]["control_id"]] is False


def test_export_accepts_unconnected_widgets_and_force_input_socket() -> None:
    """Optional sockets and omitted workflow input entries are not widgets."""

    cube = _export_single_node(
        class_type=PROMPT_CLASS,
        prompt_inputs={"positive_prompt": "", "negative_prompt": ""},
        workflow_inputs=[
            {"name": "model", "type": "MODEL", "link": None},
            {"name": "clip", "type": "CLIP", "link": None},
            {
                "name": "encode_style",
                "type": "STRING",
                "shape": 7,
                "link": None,
            },
            {
                "name": "positive_prompt",
                "type": "STRING",
                "widget": {"name": "positive_prompt"},
                "link": 10,
            },
        ],
        widget_values=["", ""],
    )

    controls = _controls_by_input(cube)

    assert {"positive_prompt", "negative_prompt"} <= controls.keys()
    assert "encode_style" not in controls


def _export_single_node(
    *,
    class_type: str,
    prompt_inputs: dict[str, Any],
    workflow_inputs: list[dict[str, Any]],
    widget_values: list[Any],
) -> dict[str, Any]:
    """Export one output-bounded node using a Comfy v3 workflow shape."""

    cube_id = "local/example-user/widget-identity.cube"
    prompt = {
        "1": {"class_type": class_type, "inputs": prompt_inputs},
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Widget Identity",
                "value": ["1", 0],
            },
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 1,
                "type": class_type,
                "pos": [100, 100],
                "size": [320, 150],
                "inputs": workflow_inputs,
                "widgets_values": widget_values,
            },
            {
                "id": 2,
                "type": "SugarCubes.CubeOutput",
                "pos": [460, 100],
                "size": [140, 46],
            },
        ],
        "links": [],
        "version": 1,
    }

    return export_cubes(
        prompt,
        workflow=workflow,
        definition_resolver=_definition,
    )[0].cube


def _definition(class_type: str) -> dict[str, Any]:
    """Return live-definition fixtures for the observed SimpleSyrup nodes."""

    if class_type == CHECKPOINT_CLASS:
        return {
            "input": {
                "required": {
                    "ckpt_name": [["machine-a.safetensors"], {}],
                    "vae_name": [
                        ["Use Checkpoint VAE", "machine-vae.safetensors"],
                        {"default": "Use Checkpoint VAE"},
                    ],
                    "clip_skip": ["BOOLEAN", {"default": False}],
                }
            },
            "input_order": {"required": ["ckpt_name", "vae_name", "clip_skip"]},
            "output": ["MODEL", "CLIP", "VAE"],
            "output_name": ["model", "clip", "vae"],
            "output_is_list": [False, False, False],
        }
    if class_type == PROMPT_CLASS:
        return {
            "input": {
                "required": {
                    "model": ["MODEL"],
                    "clip": ["CLIP"],
                    "positive_prompt": ["STRING", {"multiline": True}],
                    "negative_prompt": ["STRING", {"multiline": True}],
                },
                "optional": {
                    "encode_style": [
                        "STRING",
                        {"default": "", "forceInput": True},
                    ]
                },
            },
            "input_order": {
                "required": [
                    "model",
                    "clip",
                    "positive_prompt",
                    "negative_prompt",
                ],
                "optional": ["encode_style"],
            },
            "output": ["MODEL", "CONDITIONING", "CONDITIONING"],
            "output_name": ["model", "positive", "negative"],
            "output_is_list": [False, False, False],
        }
    return {}


def _controls_by_input(cube: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Index serialized surface controls by their stable input names."""

    return {
        control["input_name"]: control
        for control in cube["surface"]["controls"]
        if isinstance(control, dict)
    }
