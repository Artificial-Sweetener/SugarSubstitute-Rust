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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Protect current-node defaults when saved Cubes predate added fields."""

from __future__ import annotations

from sugarcubes.execution.live_node_defaults import LiveNodeDefaultReconciler


def test_added_widget_field_uses_current_declared_node_default() -> None:
    """Supply a new live field without disturbing persisted named values."""

    reconciler = LiveNodeDefaultReconciler(
        lambda class_type: (
            {
                "input": {
                    "required": {
                        "diffusion_model": [["anima.safetensors"]],
                        "quantization": [
                            ["Original", "FP8 E4M3"],
                            {"default": "Original"},
                        ],
                    }
                }
            }
            if class_type == "SimpleSyrup.SimpleLoadAnima"
            else None
        )
    )
    prompt: dict[str, dict[str, object]] = {
        "models": {
            "class_type": "SimpleSyrup.SimpleLoadAnima",
            "inputs": {"diffusion_model": "anima.safetensors"},
        }
    }

    reconciler.apply(prompt)

    assert prompt["models"]["inputs"] == {
        "diffusion_model": "anima.safetensors",
        "quantization": "Original",
    }


def test_saved_named_value_wins_over_current_node_default() -> None:
    """Preserve Cube-authored declarations above the live-node fallback."""

    reconciler = LiveNodeDefaultReconciler(
        lambda _class_type: {
            "input": {
                "required": {
                    "quantization": [
                        ["Original", "FP8 E4M3"],
                        {"default": "Original"},
                    ]
                }
            }
        }
    )
    prompt: dict[str, dict[str, object]] = {
        "models": {
            "class_type": "SimpleSyrup.SimpleLoadAnima",
            "inputs": {"quantization": "FP8 E4M3"},
        }
    }

    reconciler.apply(prompt)

    assert prompt["models"]["inputs"] == {"quantization": "FP8 E4M3"}


def test_missing_connection_input_is_not_fabricated() -> None:
    """Leave resource edges available for inheritance and Comfy validation."""

    reconciler = LiveNodeDefaultReconciler(
        lambda _class_type: {"input": {"required": {"model": ["MODEL"]}}}
    )
    prompt: dict[str, dict[str, object]] = {
        "sampler": {"class_type": "KSampler", "inputs": {}}
    }

    reconciler.apply(prompt)

    assert prompt["sampler"]["inputs"] == {}
