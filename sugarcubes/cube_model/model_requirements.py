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
"""Validate portable model policy without binding Cube data to installed provider catalogs."""

from typing import TypedDict


class CubeModelRequirement(TypedDict):
    """Keep architecture and family identities portable across host installations."""

    schemaVersion: int
    architecture: str
    family: str


def normalize_model_requirement(value: object) -> CubeModelRequirement:
    """Retain future family identities and explicit Unfiltered assignments within bounded data."""
    if not isinstance(value, dict) or set(value) != {
        "schemaVersion",
        "architecture",
        "family",
    }:
        raise ValueError(
            "metadata.model_requirements must be a versioned model requirement"
        )
    architecture, family = value.get("architecture"), value.get("family")
    if type(value.get("schemaVersion")) is not int or value["schemaVersion"] != 1:
        raise ValueError("Unsupported Cube model requirement version")
    if not isinstance(architecture, str) or not isinstance(family, str):
        raise ValueError("Cube architecture and family must be strings")
    if any(
        len(label) > 128 or any(ord(character) < 32 for character in label)
        for label in (architecture, family)
    ):
        raise ValueError("Invalid Cube model requirement label")
    return {
        "schemaVersion": 1,
        "architecture": architecture.strip(),
        "family": family.strip(),
    }
