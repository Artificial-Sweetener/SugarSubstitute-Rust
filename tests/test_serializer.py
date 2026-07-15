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

from __future__ import annotations

from collections.abc import Iterator, Mapping
from typing import Any
import re
import subprocess
import sys
from pathlib import Path

import pytest

from sugarcubes.exporter import export_cubes
from sugarcubes.exporter import definition_snapshot
from sugarcubes.exporter.layout_serializer import coerce_float, coerce_int_value
from sugarcubes.exporter.serializer import (
    BINDING_SENTINEL,
    _validate_authored_values_against_definitions,
)


def _walk_json(value: Any) -> Iterator[Any]:
    if isinstance(value, dict):
        yield value
        for entry in value.values():
            yield from _walk_json(entry)
    elif isinstance(value, list):
        yield value
        for entry in value:
            yield from _walk_json(entry)


def _contains_nested_choice_array(value: Any) -> Any:
    if isinstance(value, list):
        if value and isinstance(value[0], list):
            return True
        return any(_contains_nested_choice_array(entry) for entry in value)
    if isinstance(value, dict):
        return any(_contains_nested_choice_array(entry) for entry in value.values())
    return False


def _definition_resolver(_class_type: Any) -> Any:
    return {}


def _compact_definition_resolver(class_type: Any) -> Any:
    if class_type == "KSampler":
        return {
            "input": {
                "required": {
                    "model": ["MODEL"],
                    "latent_image": ["LATENT"],
                    "seed": ["INT", {"default": 1, "min": 0, "max": 999, "step": 1}],
                    "sampler_name": [
                        ["euler", "heun"],
                        {"default": "euler", "tooltip": "Sampler"},
                    ],
                    "scheduler": [["normal", "karras"], {"default": "normal"}],
                }
            },
            "input_order": {
                "required": [
                    "model",
                    "latent_image",
                    "seed",
                    "sampler_name",
                    "scheduler",
                ]
            },
            "output": ["LATENT"],
            "output_name": ["LATENT"],
            "output_is_list": [False],
            "output_tooltips": ["Denoised latent"],
            "description": "Sampling node help text",
            "python_module": "nodes",
            "category": "sampling",
            "display_name": "KSampler",
        }
    if class_type == "CheckpointLoaderSimple":
        return {
            "input": {
                "required": {
                    "ckpt_name": [
                        ["local-a.safetensors", "local-b.safetensors"],
                        {"default": "local-a.safetensors"},
                    ]
                }
            },
            "input_order": {"required": ["ckpt_name"]},
            "output": ["MODEL", "CLIP", "VAE"],
            "output_name": ["MODEL", "CLIP", "VAE"],
            "output_is_list": [False, False, False],
            "output_tooltips": ["Model", "CLIP", "VAE"],
            "python_module": "nodes",
        }
    if class_type == "LoadImage":
        return {
            "input": {
                "required": {
                    "image": [
                        ["before.png", "after.png"],
                        {"image_upload": True, "default": "before.png"},
                    ]
                }
            },
            "input_order": {"required": ["image"]},
            "output": ["IMAGE", "MASK"],
            "output_name": ["IMAGE", "MASK"],
            "output_is_list": [False, False],
            "python_module": "nodes",
        }
    if class_type == "LoadImageMask":
        return {
            "input": {
                "required": {
                    "image": [
                        ["mask-a.png", "mask-b.png"],
                        {"image_upload": True, "default": "mask-a.png"},
                    ],
                    "channel": [
                        ["alpha", "red", "green", "blue"],
                        {"default": "alpha"},
                    ],
                }
            },
            "input_order": {"required": ["image", "channel"]},
            "output": ["MASK"],
            "output_name": ["MASK"],
            "output_is_list": [False],
            "python_module": "nodes",
        }
    if class_type == "VAELoader":
        return {
            "input": {
                "required": {
                    "vae_name": [
                        ["local-vae.safetensors", "other-vae.safetensors"],
                        {"default": "local-vae.safetensors"},
                    ]
                }
            },
            "input_order": {"required": ["vae_name"]},
            "output": ["VAE"],
            "output_name": ["VAE"],
            "output_is_list": [False],
            "python_module": "nodes",
        }
    if class_type == "UpscaleModelLoader":
        return {
            "input": {
                "required": {
                    "model_name": [
                        ["RealESRGAN_x4.pth", "Anime6B.pth"],
                        {"default": "RealESRGAN_x4.pth"},
                    ]
                }
            },
            "input_order": {"required": ["model_name"]},
            "output": ["UPSCALE_MODEL"],
            "output_name": ["UPSCALE_MODEL"],
            "output_is_list": [False],
            "python_module": "comfy_extras.nodes_upscale_model",
        }
    if class_type == "UltralyticsDetectorProvider":
        return {
            "input": {
                "required": {
                    "model_name": [
                        ["segm/head.pt", "bbox/person.pt"],
                        {"default": "segm/head.pt"},
                    ]
                }
            },
            "input_order": {"required": ["model_name"]},
            "output": ["BBOX_DETECTOR", "SEGM_DETECTOR"],
            "output_name": ["BBOX_DETECTOR", "SEGM_DETECTOR"],
            "output_is_list": [False, False],
            "python_module": "custom_nodes.comfyui-impact-subpack",
        }
    if class_type == "SimpleSyrup.GroundingDINOModelLoader":
        return {
            "input": {
                "required": {
                    "grounding_dino_model": [
                        ["GroundingDINO_SwinT_OGC"],
                        {"default": "GroundingDINO_SwinT_OGC"},
                    ],
                    "text_encoder": [
                        ["BERT base uncased (auto)"],
                        {"default": "BERT base uncased (auto)"},
                    ],
                }
            },
            "input_order": {"required": ["grounding_dino_model", "text_encoder"]},
            "output": ["GROUNDING_DINO_MODEL"],
            "output_name": ["grounding_dino_model"],
            "output_is_list": [False],
            "python_module": "custom_nodes.SimpleSyrup",
        }
    if class_type == "SimpleSyrup.SAMModelLoader":
        return {
            "input": {
                "required": {
                    "sam_model": [
                        ["sam_vit_b"],
                        {"default": "sam_vit_b"},
                    ]
                }
            },
            "input_order": {"required": ["sam_model"]},
            "output": ["SAM_MODEL"],
            "output_name": ["sam_model"],
            "output_is_list": [False],
            "python_module": "custom_nodes.SimpleSyrup",
        }
    if class_type == "SimpleSyrup.ViTMatteModelLoader":
        return {
            "input": {
                "required": {
                    "vitmatte_model": [
                        ["vitmatte-small-composition-1k"],
                        {"default": "vitmatte-small-composition-1k"},
                    ]
                }
            },
            "input_order": {"required": ["vitmatte_model"]},
            "output": ["VITMATTE_MODEL"],
            "output_name": ["vitmatte_model"],
            "output_is_list": [False],
            "python_module": "custom_nodes.SimpleSyrup",
        }
    if class_type == "VectorscopeCC":
        return {
            "input": {
                "required": {
                    "brightness": ["FLOAT", {"default": 0.0}],
                    "contrast": ["FLOAT", {"default": 1.0}],
                    "saturation": ["FLOAT", {"default": 1.0}],
                    "r": ["FLOAT", {"default": 1.0}],
                    "g": ["FLOAT", {"default": 1.0}],
                    "b": ["FLOAT", {"default": 1.0}],
                    "alt": ["BOOLEAN", {"default": True}],
                }
            },
            "input_order": {
                "required": [
                    "brightness",
                    "contrast",
                    "saturation",
                    "r",
                    "g",
                    "b",
                    "alt",
                ]
            },
            "output": ["IMAGE"],
            "output_name": ["IMAGE"],
            "output_is_list": [False],
            "python_module": "custom_nodes.ComfyUI-Image-Filters",
        }
    if class_type == "SeedVR2LoadDiTModel":
        return {
            "input": {
                "required": {
                    "model": [
                        "COMBO",
                        {
                            "default": "seedvr2_ema_3b_fp8_e4m3fn.safetensors",
                            "options": [
                                "seedvr2_ema_3b-Q4_K_M.gguf",
                                "seedvr2_ema_3b_fp8_e4m3fn.safetensors",
                            ],
                            "tooltip": "Models automatically download on first use.",
                        },
                    ],
                    "device": [
                        "COMBO",
                        {
                            "default": "cuda:0",
                            "options": ["cuda:0"],
                            "tooltip": "GPU device.",
                        },
                    ],
                },
                "optional": {
                    "attention_mode": [
                        "COMBO",
                        {
                            "default": "sdpa",
                            "options": ["sdpa", "flash_attn_2"],
                        },
                    ]
                },
            },
            "input_order": {
                "required": ["model", "device"],
                "optional": ["attention_mode"],
            },
            "output": ["SEEDVR2_DIT"],
            "output_name": ["SEEDVR2_DIT"],
            "output_is_list": [False],
            "output_tooltips": ["DiT config"],
            "python_module": "custom_nodes.seedvr2_videoupscaler",
        }
    return {}


def _detail_segs_by_scale_factor_definition() -> Any:
    return {
        "input": {
            "required": {
                "image": ["IMAGE", {}],
                "segs": ["SEGS", {}],
                "model": ["MODEL", {}],
                "vae": ["VAE", {}],
                "positive": ["CONDITIONING,CONDITIONING_BATCH", {}],
                "negative": ["CONDITIONING,CONDITIONING_BATCH", {}],
                "scale_factor": ["FLOAT", {"default": 1.5}],
                "upscale_method": ["LIST"],
                "clamp_size": ["INT", {"default": 0}],
                "seed": [
                    "INT",
                    {"default": 0, "control_after_generate": True},
                ],
                "steps": ["INT", {"default": 20}],
                "cfg": ["FLOAT", {"default": 8.0}],
                "sampler_name": ["LIST"],
                "scheduler": ["LIST"],
                "denoise": ["FLOAT", {"default": 0.5}],
                "feather": ["INT", {"default": 5}],
                "noise_mask": ["BOOLEAN", {"default": True}],
                "noise_mask_feather": ["INT", {"default": 20}],
                "tiled_encode": ["BOOLEAN", {"default": False}],
                "tiled_decode": ["BOOLEAN", {"default": False}],
            }
        },
        "input_order": {
            "required": [
                "image",
                "segs",
                "model",
                "vae",
                "positive",
                "negative",
                "scale_factor",
                "upscale_method",
                "clamp_size",
                "seed",
                "steps",
                "cfg",
                "sampler_name",
                "scheduler",
                "denoise",
                "feather",
                "noise_mask",
                "noise_mask_feather",
                "tiled_encode",
                "tiled_decode",
            ]
        },
        "output": ["IMAGE"],
        "output_name": ["image"],
        "output_is_list": [False],
        "python_module": "custom_nodes.SimpleSyrup",
    }


def _detailer_definition_resolver(class_type: Any) -> Any:
    if class_type == "SimpleSyrup.DetailSEGSByScaleFactor":
        return _detail_segs_by_scale_factor_definition()
    return _compact_definition_resolver(class_type)


def _detailer_export_prompt(cube_id: Any) -> Any:
    return {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Detailer"},
        },
        "2": {
            "class_type": "SimpleSyrup.DetailSEGSByScaleFactor",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Detailer",
                "value": ["2", 0],
            },
        },
    }


def _detailer_export_workflow(
    widget_values: Any, *, include_snapshot: Any = True
) -> Any:
    widget_names = [
        "scale_factor",
        "upscale_method",
        "clamp_size",
        "seed",
        "steps",
        "cfg",
        "sampler_name",
        "scheduler",
        "denoise",
        "feather",
        "noise_mask",
        "noise_mask_feather",
        "tiled_encode",
        "tiled_decode",
    ]
    named_values = {}
    if include_snapshot:
        value_index = 0
        for widget_name in widget_names:
            named_values[widget_name] = widget_values[value_index]
            value_index += 1
            if widget_name == "seed" and value_index < len(widget_values):
                if widget_values[value_index] in {
                    "fixed",
                    "increment",
                    "decrement",
                    "randomize",
                }:
                    value_index += 1
    detailer_node = {
        "id": 2,
        "type": "SimpleSyrup.DetailSEGSByScaleFactor",
        "pos": [200, 0],
        "size": [290, 520],
        "widgets_values": widget_values,
    }
    if include_snapshot:
        detailer_node["sugarcubes_widget_values"] = named_values
    return {
        "nodes": [
            {
                "id": 1,
                "type": "SugarCubes.CubeInput",
                "pos": [0, 0],
                "size": [140, 46],
            },
            detailer_node,
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [520, 0],
                "size": [140, 46],
            },
        ],
        "version": 1,
    }


def _control_id_by_input(cube: Any, input_name: Any) -> Any:
    for control in cube["surface"]["controls"]:
        if (
            control["class_type"] == "SimpleSyrup.DetailSEGSByScaleFactor"
            and control["input_name"] == input_name
        ):
            return control["control_id"]
    raise AssertionError(f"Missing detailer control for {input_name}")


def _compact_definition_prompt() -> Any:
    cube_id = "local/example-user/compact.cube"
    return {
        "1": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["2", 0],
            },
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {
                "model": ["3", 0],
                "latent_image": ["4", 0],
                "seed": 7,
                "sampler_name": "euler",
                "scheduler": "normal",
            },
        },
        "3": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": {"ckpt_name": "local-a.safetensors"},
        },
        "4": {"class_type": "LoadImage", "inputs": {"image": "before.png"}},
        "5": {
            "class_type": "LoadImageMask",
            "inputs": {"image": "mask-a.png", "channel": "alpha"},
        },
        "6": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["5", 0],
            },
        },
        "7": {
            "class_type": "VAELoader",
            "inputs": {"vae_name": "local-vae.safetensors"},
        },
        "8": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["7", 0],
            },
        },
        "9": {
            "class_type": "UpscaleModelLoader",
            "inputs": {"model_name": "RealESRGAN_x4.pth"},
        },
        "10": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["9", 0],
            },
        },
        "11": {
            "class_type": "UltralyticsDetectorProvider",
            "inputs": {"model_name": "segm/head.pt"},
        },
        "12": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["11", 0],
            },
        },
        "13": {
            "class_type": "SimpleSyrup.GroundingDINOModelLoader",
            "inputs": {
                "grounding_dino_model": "GroundingDINO_SwinT_OGC",
                "text_encoder": "BERT base uncased (auto)",
            },
        },
        "14": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["13", 0],
            },
        },
        "15": {
            "class_type": "SimpleSyrup.SAMModelLoader",
            "inputs": {"sam_model": "sam_vit_b"},
        },
        "16": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["15", 0],
            },
        },
        "17": {
            "class_type": "SimpleSyrup.ViTMatteModelLoader",
            "inputs": {"vitmatte_model": "vitmatte-small-composition-1k"},
        },
        "18": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["17", 0],
            },
        },
        "19": {
            "class_type": "VectorscopeCC",
            "inputs": {
                "brightness": 0.1,
                "contrast": 1.2,
                "saturation": 0.9,
                "r": 1.0,
                "g": 0.8,
                "b": 0.7,
                "alt": True,
            },
        },
        "20": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["19", 0],
            },
        },
        "21": {
            "class_type": "SeedVR2LoadDiTModel",
            "inputs": {
                "model": "seedvr2_ema_3b_fp8_e4m3fn.safetensors",
                "device": "cuda:0",
                "attention_mode": "sdpa",
            },
        },
        "22": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Compact",
                "value": ["21", 0],
            },
        },
    }


def test_serializer_remaps_marker_inputs_to_bindings() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0], "seed": 1},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }

    cubes = export_cubes(prompt, definition_resolver=_definition_resolver)
    payload = cubes[0].cube

    node_symbol = next(iter(payload["implementation"]["nodes"]))
    node_payload = payload["implementation"]["nodes"][node_symbol]
    binding = node_payload["inputs"]["image"]

    assert binding[0] == BINDING_SENTINEL
    assert binding[1] in payload["implementation"]["inputs"]
    assert re.match(r"^input\.[a-z0-9_]+(\d+)?$", binding[1]) is not None


def test_serializer_layout_groups_and_flags() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 2,
                "type": "KSampler",
                "pos": [100, 100],
                "size": [180, 60],
                "flags": {"collapsed": True},
                "color": "#123456",
            },
        ],
        "groups": [
            {
                "title": "In Group",
                "bounding": [50, 50, 400, 300],
                "color": "#aaa",
            },
            {
                "title": "Empty Group",
                "bounding": [1000, 1000, 10, 10],
                "color": "#bbb",
            },
        ],
        "version": 1,
        "extra": {"ds": {"scale": 1.25, "offset": [10, 20]}},
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    layout = cubes[0].cube["implementation"]["layout"]

    node_layout = next(iter(layout["nodes"].values()))
    assert node_layout["flags"]["collapsed"] is True
    assert node_layout["style"]["color"] == "#123456"
    assert layout["ds"]["scale"] == 1.25
    assert len(layout["groups"]) == 1
    assert layout["groups"][0]["title"] == "In Group"


def test_serializer_persists_non_default_node_execution_mode() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {"class_type": "VAELoader", "inputs": {"vae_name": "demo.vae"}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 2,
                "type": "VAELoader",
                "pos": [100, 100],
                "size": [180, 60],
                "mode": 4,
            },
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    payload = cubes[0].cube
    node_payload = next(iter(payload["implementation"]["nodes"].values()))
    layout_node = next(iter(payload["implementation"]["layout"]["nodes"].values()))

    assert node_payload["mode"] == 4
    assert "mode" not in layout_node


def test_serializer_omits_default_or_invalid_node_execution_modes() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {"class_type": "KSampler", "inputs": {"image": ["2", 0]}},
        "4": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["3", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 2,
                "type": "KSampler",
                "pos": [100, 100],
                "size": [180, 60],
                "mode": 0,
            },
            {
                "id": 3,
                "type": "KSampler",
                "pos": [320, 100],
                "size": [180, 60],
                "mode": "4",
            },
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    nodes = cubes[0].cube["implementation"]["nodes"].values()

    assert all("mode" not in node for node in nodes)


def test_serializer_canonicalizes_existing_cube_identity_from_lookup() -> None:
    cube_id = "Artificial-Sweetener/Base-Cubes/automask detailer.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": cube_id},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": cube_id, "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {"id": 1, "type": "SugarCubes.CubeInput", "pos": [0, 0], "size": [140, 46]},
            {"id": 2, "type": "KSampler", "pos": [200, 0], "size": [180, 60]},
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [420, 0],
                "size": [140, 46],
            },
        ],
        "groups": [
            {
                "title": f"{cube_id} 2",
                "bounding": [0, 0, 640, 200],
                "sugarcubes": {
                    "schema": 2,
                    "instance_id": "inst-1",
                    "cube_id": cube_id,
                    "default_alias": cube_id,
                    "alias": f"{cube_id} 2",
                },
            }
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt,
        workflow=workflow,
        definition_resolver=_definition_resolver,
        default_alias_lookup={cube_id: "automask detailer"},
    )
    payload = cubes[0].cube
    group = payload["implementation"]["layout"]["groups"][0]

    assert payload["metadata"]["default_alias"] == "automask detailer"
    assert group["title"] == "automask detailer"
    assert group["sugarcubes"]["default_alias"] == "automask detailer"
    assert "alias" not in group["sugarcubes"]


def test_serializer_persists_route_default_alias_without_lookup() -> None:
    cube_id = "Artificial-Sweetener/Base-Cubes/image to image.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Image to Image"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Image to Image",
                "value": ["2", 0],
            },
        },
    }

    cubes = export_cubes(prompt, definition_resolver=_definition_resolver)

    assert cubes[0].cube["metadata"]["default_alias"] == "image to image"


def test_serializer_compacts_list_definitions_and_removes_help_metadata() -> None:
    cubes = export_cubes(
        _compact_definition_prompt(),
        definition_resolver=_compact_definition_resolver,
    )
    definitions = cubes[0].cube["implementation"]["definitions"]

    ksampler = definitions["KSampler"]
    assert ksampler["input"]["required"]["sampler_name"] == ["LIST"]
    assert ksampler["input"]["required"]["scheduler"] == ["LIST"]
    assert ksampler["input"]["required"]["seed"] == [
        "INT",
        {"default": 1, "min": 0, "max": 999, "step": 1},
    ]
    assert ksampler["input_order"]["required"] == [
        "model",
        "latent_image",
        "seed",
        "sampler_name",
        "scheduler",
    ]
    assert ksampler["output"] == ["LATENT"]
    assert ksampler["output_name"] == ["LATENT"]
    assert ksampler["output_is_list"] == [False]
    assert ksampler["python_module"] == "nodes"
    assert "description" not in ksampler
    assert "output_tooltips" not in ksampler

    assert definitions["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"] == [
        "LIST"
    ]
    assert definitions["LoadImage"]["input"]["required"]["image"] == ["LIST"]
    assert definitions["LoadImageMask"]["input"]["required"]["image"] == ["LIST"]
    assert definitions["LoadImageMask"]["input"]["required"]["channel"] == ["LIST"]
    assert definitions["SeedVR2LoadDiTModel"]["input"]["required"]["model"] == ["LIST"]
    assert definitions["SeedVR2LoadDiTModel"]["input"]["required"]["device"] == ["LIST"]
    assert definitions["SeedVR2LoadDiTModel"]["input"]["optional"][
        "attention_mode"
    ] == ["LIST"]
    for entry in _walk_json(definitions):
        if isinstance(entry, dict):
            assert "tooltip" not in entry
            assert "output_tooltips" not in entry
            assert "description" not in entry
            assert "options" not in entry
    assert not _contains_nested_choice_array(definitions)


def test_serializer_preserves_only_portable_authored_picker_defaults() -> None:
    cubes = export_cubes(
        _compact_definition_prompt(),
        definition_resolver=_compact_definition_resolver,
    )
    payload = cubes[0].cube
    controls = payload["surface"]["controls"]
    authored_values = payload["flavors"]["authored"][0]["values"]

    local_resource_fields = {
        ("CheckpointLoaderSimple", "ckpt_name"),
        ("LoadImage", "image"),
        ("LoadImageMask", "image"),
        ("VAELoader", "vae_name"),
        ("UpscaleModelLoader", "model_name"),
        ("UltralyticsDetectorProvider", "model_name"),
        ("SeedVR2LoadDiTModel", "model"),
        ("SeedVR2LoadDiTModel", "device"),
    }
    portable_fields = {
        ("SimpleSyrup.GroundingDINOModelLoader", "grounding_dino_model"),
        ("SimpleSyrup.GroundingDINOModelLoader", "text_encoder"),
        ("SimpleSyrup.SAMModelLoader", "sam_model"),
        ("SimpleSyrup.ViTMatteModelLoader", "vitmatte_model"),
        ("VectorscopeCC", "brightness"),
        ("VectorscopeCC", "contrast"),
        ("VectorscopeCC", "saturation"),
        ("VectorscopeCC", "r"),
        ("VectorscopeCC", "g"),
        ("VectorscopeCC", "b"),
        ("VectorscopeCC", "alt"),
        ("SeedVR2LoadDiTModel", "attention_mode"),
    }

    controls_by_field = {
        (control["class_type"], control["input_name"]): control for control in controls
    }

    for field in portable_fields:
        assert controls_by_field[field]["control_id"] in authored_values
    for field in local_resource_fields:
        assert field in controls_by_field
        assert controls_by_field[field]["control_id"] not in authored_values

    assert controls_by_field[("KSampler", "seed")]["control_id"] not in authored_values
    assert (
        authored_values[controls_by_field[("LoadImageMask", "channel")]["control_id"]]
        == "alpha"
    )
    assert (
        authored_values[
            controls_by_field[
                ("SimpleSyrup.GroundingDINOModelLoader", "grounding_dino_model")
            ]["control_id"]
        ]
        == "GroundingDINO_SwinT_OGC"
    )
    assert (
        authored_values[
            controls_by_field[("SimpleSyrup.GroundingDINOModelLoader", "text_encoder")][
                "control_id"
            ]
        ]
        == "BERT base uncased (auto)"
    )


def test_serializer_backfills_missing_widget_values_from_workflow() -> None:
    cube_id = "local/example-user/widgets.cube"
    prompt = {
        "1": {
            "class_type": "KSampler",
            "inputs": {
                "model": ["3", 0],
                "latent_image": ["4", 0],
            },
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Widgets",
                "value": ["1", 0],
            },
        },
        "3": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": {"ckpt_name": "local-a.safetensors"},
        },
        "4": {"class_type": "LoadImage", "inputs": {"image": "before.png"}},
    }
    workflow = {
        "nodes": [
            {
                "id": 1,
                "type": "KSampler",
                "pos": [100, 100],
                "size": [180, 60],
                "widgets_values": [7, "euler", "normal"],
                "sugarcubes_widget_values": {
                    "seed": 7,
                    "sampler_name": "euler",
                    "scheduler": "normal",
                },
            },
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt,
        workflow=workflow,
        definition_resolver=_compact_definition_resolver,
    )
    authored_values = cubes[0].cube["flavors"]["authored"][0]["values"]

    assert authored_values["ksampler.sampler_name"] == "euler"
    assert authored_values["ksampler.scheduler"] == "normal"
    assert "ksampler.seed" not in authored_values


def test_serializer_rejects_ambiguous_positional_widget_values() -> None:
    cube_id = "local/example-user/detailer-corrupt.cube"

    with pytest.raises(ValueError, match="Unsafe workflow widget snapshot"):
        export_cubes(
            _detailer_export_prompt(cube_id),
            workflow=_detailer_export_workflow(
                [
                    2,
                    0,
                    0,
                    "randomize",
                    7,
                    "euler_ancestral",
                    "normal",
                    0.3,
                    5,
                    True,
                    20,
                    False,
                    False,
                    False,
                ],
                include_snapshot=False,
            ),
            definition_resolver=_detailer_definition_resolver,
        )


def test_serializer_backfills_detailer_seed_control_companion_safely() -> None:
    cube_id = "local/example-user/detailer-valid.cube"

    cubes = export_cubes(
        _detailer_export_prompt(cube_id),
        workflow=_detailer_export_workflow(
            [
                2,
                "lanczos",
                0,
                123456,
                "randomize",
                7,
                7.0,
                "euler_ancestral",
                "normal",
                0.3,
                5,
                True,
                20,
                False,
                False,
            ]
        ),
        definition_resolver=_detailer_definition_resolver,
    )
    payload = cubes[0].cube
    authored_values = payload["flavors"]["authored"][0]["values"]

    assert _control_id_by_input(payload, "seed") not in authored_values
    assert authored_values[_control_id_by_input(payload, "steps")] == 7
    assert authored_values[_control_id_by_input(payload, "cfg")] == 7.0
    assert (
        authored_values[_control_id_by_input(payload, "sampler_name")]
        == "euler_ancestral"
    )
    assert authored_values[_control_id_by_input(payload, "scheduler")] == "normal"
    assert authored_values[_control_id_by_input(payload, "denoise")] == 0.3
    assert authored_values[_control_id_by_input(payload, "feather")] == 5
    assert authored_values[_control_id_by_input(payload, "noise_mask")] is True
    assert authored_values[_control_id_by_input(payload, "noise_mask_feather")] == 20


def test_serializer_rejects_authored_values_that_contradict_definitions() -> None:
    cube_id = "local/example-user/detailer-validator.cube"
    cubes = export_cubes(
        _detailer_export_prompt(cube_id),
        workflow=_detailer_export_workflow(
            [
                2,
                "lanczos",
                0,
                123456,
                "randomize",
                7,
                7.0,
                "euler_ancestral",
                "normal",
                0.3,
                5,
                True,
                20,
                False,
                False,
            ]
        ),
        definition_resolver=_detailer_definition_resolver,
    )
    payload = cubes[0].cube
    payload["flavors"]["authored"][0]["values"][
        _control_id_by_input(payload, "cfg")
    ] = "euler_ancestral"

    with pytest.raises(ValueError, match="control_id=.*cfg"):
        _validate_authored_values_against_definitions(payload)


def test_serializer_preserves_explicit_blank_text_from_workflow() -> None:
    cube_id = "local/example-user/blank-prompt.cube"

    def resolver(class_type: Any) -> Any:
        if class_type == "PrimitiveStringMultiline":
            return {
                "input": {"required": {"value": ["STRING", {"multiline": True}]}},
                "input_order": {"required": ["value"]},
                "output": ["STRING"],
                "output_name": ["STRING"],
                "output_is_list": [False],
            }
        return _compact_definition_resolver(class_type)

    prompt = {
        "1": {"class_type": "PrimitiveStringMultiline", "inputs": {}},
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Blank Prompt",
                "value": ["1", 0],
            },
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 1,
                "type": "PrimitiveStringMultiline",
                "pos": [100, 100],
                "size": [180, 60],
                "widgets_values": [""],
                "sugarcubes_widget_values": {"value": ""},
            },
        ],
        "version": 1,
    }

    cubes = export_cubes(prompt, workflow=workflow, definition_resolver=resolver)
    authored_values = cubes[0].cube["flavors"]["authored"][0]["values"]

    assert authored_values == {"primitivestringmultiline.value": ""}


def test_serializer_does_not_synthesize_empty_picker_defaults() -> None:
    cube_id = "local/example-user/missing-picker.cube"
    prompt = {
        "1": {"class_type": "CheckpointLoaderSimple", "inputs": {}},
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Missing Picker",
                "value": ["1", 0],
            },
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 1,
                "type": "CheckpointLoaderSimple",
                "pos": [100, 100],
                "size": [180, 60],
            },
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt,
        workflow=workflow,
        definition_resolver=_compact_definition_resolver,
    )
    authored_values = cubes[0].cube["flavors"]["authored"][0]["values"]

    assert "checkpoint_loader_simple.ckpt_name" not in authored_values


def test_serializer_sanitizes_cube_layout_group_metadata() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {"id": 1, "type": "SugarCubes.CubeInput", "pos": [0, 0], "size": [140, 46]},
            {"id": 2, "type": "KSampler", "pos": [200, 0], "size": [180, 60]},
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [420, 0],
                "size": [140, 46],
            },
        ],
        "groups": [
            {
                "title": "Polluted Instance Alias",
                "bounding": [0, 0, 640, 200],
                "sugarcubes": {
                    "schema": 6,
                    "managed": True,
                    "cube_id": cube_id,
                    "default_alias": "Demo",
                    "target_model": "SDXL",
                    "cube_version": "2.0.0",
                    "cube_revision_ref": "current",
                    "cube_definition_key": f"{cube_id}@2.0.0",
                    "surface_signature": "surface-a",
                    "markers": {"inputs": ["1"], "outputs": ["3"]},
                    "nodes": ["2"],
                    "bounds": {"x": 0, "y": 0, "w": 640, "h": 200},
                    "alias": "Legacy Alias",
                    "instance_id": "inst-1",
                    "instance_alias": "Polluted Instance Alias",
                    "flavor": "portrait",
                    "flavor_scope": "local",
                    "active_flavor_values": {"ksampler.cfg": 9},
                    "local_flavors": [{"id": "portrait"}],
                    "flavor_options": [{"id": "default"}],
                    "dirty": True,
                    "dirty_at": "2024-01-01T00:00:00Z",
                    "implementation_dirty": True,
                    "surface_values_changed": True,
                    "cosmetic_dirty": True,
                    "has_saveable_changes": True,
                },
            }
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    group_metadata = cubes[0].cube["implementation"]["layout"]["groups"][0][
        "sugarcubes"
    ]
    group = cubes[0].cube["implementation"]["layout"]["groups"][0]

    assert group["bounding"] == [-10.0, -60.0, 580.0, 130.0]
    assert group_metadata == {
        "schema": 6,
        "managed": True,
        "cube_id": cube_id,
        "default_alias": "Demo",
        "target_model": "SDXL",
        "cube_version": "1.0.0",
        "cube_revision_ref": "current",
        "cube_definition_key": f"{cube_id}@1.0.0",
        "surface_signature": "surface-a",
        "markers": {"inputs": ["1"], "outputs": ["3"]},
        "nodes": ["2"],
        "bounds": {
            "x": -10.0,
            "y": -60.0,
            "w": 580.0,
            "h": 130.0,
            "padding": {"x": 2.0, "y": 2.0, "top_extra": 0.0},
            "header": {"height": 32.0},
        },
    }


@pytest.mark.parametrize(
    "source_bounding",
    (
        [-2, -34, 564, 96],
        [-120, -160, 900, 500],
    ),
)
def test_serializer_derives_reusable_group_chrome_from_content(
    source_bounding: Any,
) -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 1,
                "type": "SugarCubes.CubeInput",
                "pos": [100, 40],
                "size": [140, 46],
            },
            {"id": 2, "type": "KSampler", "pos": [300, 40], "size": [180, 60]},
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [520, 40],
                "size": [140, 46],
            },
        ],
        "groups": [
            {
                "title": "Manual Group",
                "bounding": source_bounding,
                "sugarcubes": {
                    "schema": 6,
                    "managed": True,
                    "cube_id": cube_id,
                    "default_alias": "Demo",
                    "markers": {"inputs": ["1"], "outputs": ["3"]},
                    "nodes": ["2"],
                    "bounds": {
                        "x": source_bounding[0],
                        "y": source_bounding[1],
                        "w": source_bounding[2],
                        "h": source_bounding[3],
                    },
                },
            }
        ],
        "version": 1,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    group = cubes[0].cube["implementation"]["layout"]["groups"][0]

    assert group["bounding"] == [-10.0, -60.0, 580.0, 130.0]
    assert group["sugarcubes"]["bounds"] == {
        "x": 90.0,
        "y": -20.0,
        "w": 580.0,
        "h": 130.0,
        "padding": {"x": 2.0, "y": 2.0, "top_extra": 0.0},
        "header": {"height": 32.0},
    }


def test_serializer_accepts_empty_workflow_mapping() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }

    cubes = export_cubes(prompt, workflow={}, definition_resolver=_definition_resolver)
    warnings = cubes[0].warnings

    assert (
        "Layout metadata unavailable; frontend did not provide workflow data"
        not in warnings
    )


def test_serializer_includes_definitions_for_subgraph_node_types() -> None:
    cube_id = "local/example-user/demo.cube"
    wrapper_id = "94f725d5-39bf-4060-be68-f573214a2055"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": wrapper_id,
            "inputs": {"string": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": wrapper_id,
                    "nodes": [
                        {"id": 101, "type": "RegexExtract"},
                        {"id": 102, "type": "StringConcatenate"},
                    ],
                    "links": [],
                    "inputs": [{"name": "value", "label": "Scale Factor"}],
                    "outputs": [{"name": "image", "label": "Image"}],
                }
            ]
        }
    }

    def resolver(class_type: str) -> Any:
        if class_type == "RegexExtract":
            return {"input": {"required": {"regex_pattern": ["STRING"]}}}
        if class_type == "StringConcatenate":
            return {"input": {"required": {"delimiter": ["STRING"]}}}
        return {}

    cubes = export_cubes(prompt, workflow=workflow, definition_resolver=resolver)
    definitions = cubes[0].cube["implementation"]["definitions"]

    assert "RegexExtract" in definitions
    assert "StringConcatenate" in definitions
    subgraph = cubes[0].cube["implementation"]["subgraphs"][0]
    assert subgraph["inputs"][0]["name"] == "value"
    assert subgraph["inputs"][0]["label"] == "Scale Factor"


def test_serializer_rejects_corrupt_subgraph_widget_values() -> None:
    """Save-time validation blocks corrupted nested node widget arrays."""

    cube_id = "local/example-user/corrupt-widget.cube"
    wrapper_id = "94f725d5-39bf-4060-be68-f573214a2055"
    prompt = {
        "1": {"class_type": wrapper_id, "inputs": {}},
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Corrupt Widget",
                "value": ["1", 0],
            },
        },
    }
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": wrapper_id,
                    "nodes": [
                        {
                            "id": 101,
                            "type": "WidgetNode",
                            "inputs": [
                                {
                                    "name": "white_point",
                                    "type": "FLOAT",
                                    "widget": {"name": "white_point"},
                                    "link": None,
                                }
                            ],
                            "outputs": [],
                            "widgets_values": [2048],
                        }
                    ],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                }
            ]
        }
    }

    def resolver(class_type: str) -> Any:
        if class_type == "WidgetNode":
            return {
                "input": {
                    "required": {
                        "white_point": [
                            "FLOAT",
                            {"default": 0.99, "min": 0.02, "max": 1.0},
                        ]
                    }
                },
                "input_order": {"required": ["white_point"]},
            }
        return {}

    with pytest.raises(ValueError, match="white_point.*maximum 1.0"):
        export_cubes(prompt, workflow=workflow, definition_resolver=resolver)


def test_serializer_backfills_subgraph_interface_label_from_name() -> None:
    cube_id = "local/example-user/demo.cube"
    wrapper_id = "94f725d5-39bf-4060-be68-f573214a2055"
    prompt = {
        "1": {
            "class_type": wrapper_id,
            "inputs": {"value": 1.5},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["1", 0]},
        },
    }
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": wrapper_id,
                    "nodes": [{"id": 101, "type": "RegexExtract"}],
                    "links": [],
                    "inputs": [{"name": "value"}],
                    "outputs": [],
                }
            ]
        }
    }

    cubes = export_cubes(prompt, workflow=workflow, definition_resolver=lambda _: {})
    subgraph = cubes[0].cube["implementation"]["subgraphs"][0]

    assert subgraph["inputs"][0]["label"] == "value"


def test_serializer_rejects_duplicate_subgraph_interface_labels() -> None:
    cube_id = "local/example-user/demo.cube"
    wrapper_id = "94f725d5-39bf-4060-be68-f573214a2055"
    prompt = {
        "1": {
            "class_type": wrapper_id,
            "inputs": {"value": 1.5},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["1", 0]},
        },
    }
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": wrapper_id,
                    "nodes": [{"id": 101, "type": "RegexExtract"}],
                    "links": [],
                    "inputs": [
                        {"name": "value", "label": "Scale"},
                        {"name": "factor", "label": "Scale"},
                    ],
                    "outputs": [],
                }
            ]
        }
    }

    with pytest.raises(ValueError, match="label 'Scale'"):
        export_cubes(prompt, workflow=workflow, definition_resolver=lambda _: {})


def test_serializer_reports_definition_lookup_failures_as_warnings(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }

    def resolver(_class_type: str) -> Mapping[str, Any]:
        raise RuntimeError("resolver unavailable")

    def fail_live_lookup(_class_type: str) -> Mapping[str, Any]:
        raise AssertionError("resolver failures must not load live Comfy definitions")

    monkeypatch.setattr(
        definition_snapshot,
        "resolve_definition_via_nodes",
        fail_live_lookup,
    )

    cubes = export_cubes(prompt, definition_resolver=resolver)

    assert any(
        "Definition lookup failed for 'KSampler': resolver unavailable" == warning
        for warning in cubes[0].warnings
    )


def test_serializer_coercion_helpers_preserve_current_fallbacks() -> None:
    assert coerce_float("1.5", 0.0) == 1.5
    assert coerce_float("bad", 2.0) == 2.0
    assert coerce_int_value(3.9) == 3
    assert coerce_int_value("4") == 4
    assert coerce_int_value("4.8") == 4
    assert coerce_int_value("bad") is None


def test_resolve_definition_via_nodes_degrades_when_runtime_is_missing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        definition_snapshot, "_load_comfy_runtime", lambda: (None, None)
    )

    assert definition_snapshot.resolve_definition_via_nodes("KSampler") is None


def test_serializer_import_does_not_load_comfy_nodes() -> None:
    script = (
        "import sys; "
        "sys.path.insert(0, r'.'); "
        "import sugarcubes.exporter.serializer; "
        "raise SystemExit(1 if 'nodes' in sys.modules else 0)"
    )

    result = subprocess.run(
        [sys.executable, "-c", script],
        check=False,
        cwd=Path(__file__).resolve().parents[1],
    )

    assert result.returncode == 0
