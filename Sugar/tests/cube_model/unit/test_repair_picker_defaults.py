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
"""Tests for the local cube picker-default repair utility."""

from __future__ import annotations

from typing import Any

from scripts.repair_picker_defaults import repair_cube_payload


class _FakeObjectInfoClient:
    """Provide deterministic object-info definitions for repair tests."""

    def __init__(self, definitions: dict[str, dict[str, Any]]) -> None:
        """Store the definitions returned by class type."""

        self._definitions = definitions

    def definition_for(self, class_type: str) -> dict[str, Any] | None:
        """Return a test definition for one class type."""

        return self._definitions.get(class_type)


def test_repair_compacts_definitions_and_repairs_picker_blank() -> None:
    """Repair replaces picker blanks with local defaults and drops inventories."""

    payload = _cube_payload(
        definitions={
            "CheckpointLoaderSimple": {
                "input": {
                    "required": {
                        "ckpt_name": [
                            "COMBO",
                            {
                                "default": "local-a.safetensors",
                                "options": ["local-a.safetensors"],
                                "tooltip": "help",
                            },
                        ]
                    }
                },
                "description": "help",
            }
        },
        controls=[
            {
                "control_id": "checkpoint.ckpt_name",
                "class_type": "CheckpointLoaderSimple",
                "input_name": "ckpt_name",
            }
        ],
        values={"checkpoint.ckpt_name": ""},
    )
    client = _FakeObjectInfoClient(
        {
            "CheckpointLoaderSimple": {
                "input": {
                    "required": {
                        "ckpt_name": [
                            ["local-a.safetensors", "local-b.safetensors"],
                            {"default": "local-b.safetensors"},
                        ]
                    }
                }
            }
        }
    )

    repaired, notes = repair_cube_payload(payload, client=client)

    assert repaired["implementation"]["definitions"]["CheckpointLoaderSimple"]["input"][
        "required"
    ]["ckpt_name"] == ["LIST"]
    assert (
        "description"
        not in repaired["implementation"]["definitions"]["CheckpointLoaderSimple"]
    )
    assert (
        repaired["flavors"]["authored"][0]["values"]["checkpoint.ckpt_name"]
        == "local-b.safetensors"
    )
    assert "compacted definitions" in notes


def test_repair_preserves_blank_text_and_repairs_scalar_defaults() -> None:
    """Repair keeps authored text blanks while fixing invalid scalar blanks."""

    payload = _cube_payload(
        definitions={},
        controls=[
            {
                "control_id": "prompt.value",
                "class_type": "PrimitiveStringMultiline",
                "input_name": "value",
            },
            {
                "control_id": "loader.cache_model",
                "class_type": "SeedVR2LoadDiTModel",
                "input_name": "cache_model",
            },
        ],
        values={"prompt.value": "", "loader.cache_model": ""},
    )
    client = _FakeObjectInfoClient(
        {
            "PrimitiveStringMultiline": {
                "input": {"required": {"value": ["STRING", {"multiline": True}]}}
            },
            "SeedVR2LoadDiTModel": {
                "input": {"optional": {"cache_model": ["BOOLEAN", {"default": False}]}}
            },
        }
    )

    repaired, _notes = repair_cube_payload(payload, client=client)

    values = repaired["flavors"]["authored"][0]["values"]
    assert values["prompt.value"] == ""
    assert values["loader.cache_model"] is False


def test_repair_removes_picker_blank_without_local_fallback() -> None:
    """Repair removes blank picker preferences when no local fallback exists."""

    payload = _cube_payload(
        definitions={},
        controls=[
            {
                "control_id": "loader.model",
                "class_type": "CustomLoader",
                "input_name": "model",
            }
        ],
        values={"loader.model": ""},
    )
    client = _FakeObjectInfoClient(
        {"CustomLoader": {"input": {"required": {"model": ["COMBO", {"options": []}]}}}}
    )

    repaired, _notes = repair_cube_payload(payload, client=client)

    assert "loader.model" not in repaired["flavors"]["authored"][0]["values"]


def _cube_payload(
    *,
    definitions: dict[str, Any],
    controls: list[dict[str, Any]],
    values: dict[str, Any],
) -> dict[str, Any]:
    """Build the minimal current cube payload needed by repair tests."""

    return {
        "cube_id": "local/example-user/test.cube",
        "version": "1.0.0",
        "description": "",
        "surface": {"controls": controls, "default_flavor_id": "default"},
        "flavors": {
            "authored": [{"id": "default", "name": "Default", "values": values}]
        },
        "implementation": {"nodes": {}, "definitions": definitions},
    }
