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
"""Verify canonical Cube outputs preserve their exact implementation source."""

from __future__ import annotations

from typing import Any, Mapping, cast

from sugarcubes.exporter import export


def _definition_resolver(class_type: str) -> Mapping[str, Any]:
    """Return one two-output definition for binding-slot characterization."""

    return {
        "name": class_type,
        "input": {"required": {}},
        "output": ["IMAGE", "MASK"],
        "output_name": ["IMAGE", "MASK"],
        "output_is_list": [False, False],
    }


def _export_output_slot(slot: int) -> dict[str, Any]:
    """Export one marker-era Cube boundary connected to the requested slot."""

    cube_id = "local/personal/output-slot.cube"
    exported = export(
        {
            "source": {
                "class_type": "MultiOutput",
                "inputs": {},
                "_meta": {"title": "Source"},
            },
            "output": {
                "class_type": "SugarCubes.CubeOutput",
                "inputs": {
                    "cube_id": cube_id,
                    "default_alias": "Output Slot",
                    "value": ["source", slot],
                },
            },
        },
        definition_resolver=_definition_resolver,
    )[0]
    return cast(dict[str, Any], exported.cube["implementation"]["outputs"])


def test_slot_zero_output_retains_an_explicit_source_slot() -> None:
    """Canonical output bindings should not depend on implicit slot shorthand."""

    assert _export_output_slot(0) == {"output.image": ["source", 0]}


def test_nonzero_output_retains_its_exact_source_slot() -> None:
    """Multi-output nodes must never be rebound silently to slot zero."""

    assert _export_output_slot(1) == {"output.mask": ["source", 1]}
