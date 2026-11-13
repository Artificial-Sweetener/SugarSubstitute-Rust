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
"""Characterize exporter version and Comfy type resolution contracts."""

from __future__ import annotations

from copy import deepcopy

from sugarcubes.exporter.versioning import (
    VersionSuggestion,
    resolve_input_type,
    resolve_output_type_by_slot,
    suggest_version,
)


def _legacy_payload() -> dict[str, object]:
    """Build one legacy cube accepted by the exporter compatibility boundary."""

    return {
        "cube_id": "local/example-user/versioned.cube",
        "version": "2.3.4",
        "description": "Versioned cube",
        "metadata": {},
        "nodes": {
            "sampler": {
                "class_type": "KSampler",
                "inputs": {"cfg": 7.0, "model": ["@binding", "input.model"]},
            }
        },
        "inputs": {
            "input.model": {
                "kind": "input",
                "targets": [["sampler", "model"]],
            }
        },
        "outputs": {"output.image": "sampler"},
        "layout": {"origin": [0, 0], "nodes": {}, "groups": []},
        "definitions": {
            "KSampler": {
                "input": {"required": {"cfg": ["FLOAT"], "model": ["MODEL"]}},
                "output": ["IMAGE"],
            }
        },
        "subgraphs": [],
    }


def _legacy_sampler_inputs(payload: dict[str, object]) -> dict[str, object]:
    """Return the mutable legacy sampler input mapping."""

    nodes = payload["nodes"]
    assert isinstance(nodes, dict)
    sampler = nodes["sampler"]
    assert isinstance(sampler, dict)
    inputs = sampler["inputs"]
    assert isinstance(inputs, dict)
    return inputs


def test_exporter_versioning_migrates_legacy_payloads_before_comparison() -> None:
    """Legacy persisted cubes retain patch-version behavior at the public boundary."""

    old_cube = _legacy_payload()
    new_cube = deepcopy(old_cube)
    _legacy_sampler_inputs(new_cube)["cfg"] = 8.0

    suggestion = suggest_version(old_cube, new_cube)

    assert suggestion == VersionSuggestion(
        bump="patch",
        suggested="2.3.5",
        reason="Authored flavor changed",
    )


def test_exporter_versioning_preserves_legacy_no_change_version() -> None:
    """Equivalent legacy payloads keep their persisted semantic version."""

    payload = _legacy_payload()

    suggestion = suggest_version(payload, deepcopy(payload))

    assert suggestion == VersionSuggestion(
        bump="none",
        suggested="2.3.4",
        reason="No changes detected",
    )


def test_input_type_resolution_reads_required_optional_and_hidden_sections() -> None:
    """Serializer bindings resolve every Comfy input-definition section."""

    definitions = {
        "Node": {
            "input": {
                "required": {"model": ["MODEL", {"tooltip": "model"}]},
                "optional": {"strength": "FLOAT"},
                "hidden": {"prompt": ("PROMPT",)},
            }
        }
    }

    assert resolve_input_type(definitions, "Node", "model") == "MODEL"
    assert resolve_input_type(definitions, "Node", "strength") == "FLOAT"
    assert resolve_input_type(definitions, "Node", "prompt") == "PROMPT"


def test_input_type_resolution_fails_closed_for_unknown_or_malformed_definitions() -> (
    None
):
    """Unknown nodes, ports, and malformed definitions produce no invented type."""

    definitions: dict[str, object] = {"Node": {"input": {"required": {"model": []}}}}

    assert resolve_input_type(definitions, "Missing", "model") == ""
    assert resolve_input_type(definitions, "Node", "missing") == ""
    assert resolve_input_type(definitions, "Node", 0) == ""
    assert resolve_input_type([], "Node", "model") == ""


def test_output_type_resolution_supports_single_and_named_slot_outputs() -> None:
    """Serializer output bindings retain Comfy slot-index semantics."""

    definitions = {
        "Single": {"output": ["IMAGE"]},
        "Multiple": {"output": ["IMAGE", ["MASK", {"shape": "alpha"}]]},
    }

    assert resolve_output_type_by_slot(definitions, "Single", None) == "IMAGE"
    assert resolve_output_type_by_slot(definitions, "Multiple", 0) == "IMAGE"
    assert resolve_output_type_by_slot(definitions, "Multiple", 1) == "MASK"


def test_output_type_resolution_fails_closed_for_invalid_slots() -> None:
    """Malformed output declarations and invalid slots produce an empty type."""

    definitions = {"Multiple": {"output": ["IMAGE", "MASK"]}}

    assert resolve_output_type_by_slot(definitions, "Multiple", -1) == ""
    assert resolve_output_type_by_slot(definitions, "Multiple", 2) == ""
    assert resolve_output_type_by_slot(definitions, "Multiple", "1") == ""
    assert resolve_output_type_by_slot({}, "Multiple", 0) == ""
