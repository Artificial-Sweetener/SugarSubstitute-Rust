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

"""Specify human-readable implementation changes and aggregate default choices."""

from __future__ import annotations

from typing import Any

from sugarcubes.cube_model import CubeDocument, build_implementation_change_plan


def _document(
    *,
    width: int,
    prompt: str,
    nodes: dict[str, dict[str, Any]] | None = None,
) -> CubeDocument:
    """Build one cube with resolution and prompt surface controls."""

    return CubeDocument.from_dict(
        {
            "cube_id": "Artificial-Sweetener/Base-Cubes/Anima/Prompt by Region.cube",
            "version": "4.0.0",
            "metadata": {"default_alias": "Anima/Prompt by Region"},
            "implementation": {
                "nodes": nodes or {},
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {
                    "PrimitiveStringMultiline": {
                        "input": {
                            "required": {
                                "value": [
                                    "STRING",
                                    {"default": "", "multiline": True},
                                ]
                            }
                        }
                    }
                },
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": "latent.width",
                        "symbol": "latent",
                        "input_name": "width",
                        "label": "width",
                        "class_type": "EmptyLatentImage",
                        "value_type": "number",
                    },
                    {
                        "control_id": "prompt.value",
                        "symbol": "prompt",
                        "input_name": "value",
                        "label": "positive prompt",
                        "class_type": "PrimitiveStringMultiline",
                        "value_type": "string",
                    },
                ],
            },
            "flavors": {
                "authored": [
                    {
                        "id": "default",
                        "name": "Default",
                        "values": {"latent.width": width, "prompt.value": prompt},
                    }
                ]
            },
        }
    )


def test_plan_exposes_resolution_and_prompt_changes_behind_aggregate_choices() -> None:
    """Describe every possible persisted change without per-control decisions."""

    existing = _document(width=960, prompt="")
    exported = _document(width=1080, prompt="authored prompt")

    plan = build_implementation_change_plan(existing, exported)

    assert plan.display_name == "Anima/Prompt by Region"
    assert plan.overwrite_default_count == 1
    assert plan.prompt_default_count == 1
    assert any(
        change.label == "width"
        and change.previous_value == 960
        and change.proposed_value == 1080
        and change.decision == "overwrite_defaults"
        for change in plan.changes
    )
    assert any(
        change.label == "positive prompt" and change.decision == "save_prompt_fields"
        for change in plan.changes
    )


def test_plan_includes_structural_changes_as_unconditional_commit_changes() -> None:
    """Include implementation structure independently from authored defaults."""

    existing = _document(width=960, prompt="")
    exported = _document(
        width=960,
        prompt="",
        nodes={
            "sampler": {
                "class_type": "KSampler",
                "label": "Sampler",
                "inputs": {},
            }
        },
    )

    plan = build_implementation_change_plan(existing, exported)

    assert plan.requires_default_decision is False
    assert any(
        change.section == "Nodes"
        and change.label == "Sampler"
        and change.decision == "always"
        for change in plan.changes
    )
