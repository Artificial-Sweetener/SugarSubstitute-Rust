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
"""Prove additive Cube requirements survive portable serialization and reject malformed policy."""

import pytest

from sugarcubes.cube_model.document import CubeDocument, CubeSchemaError


@pytest.mark.parametrize(
    "architecture,family",
    [("SDXL", "Illustrious"), ("Future architecture", "Future family"), ("", "")],
)
def test_cube_model_requirement_roundtrip(architecture: str, family: str) -> None:
    """Retain exact portable labels including explicit Unfiltered without machine-local paths."""
    requirement = {"schemaVersion": 1, "architecture": architecture, "family": family}
    document = CubeDocument(
        "local/personal/Model.cube",
        "1.0.0",
        metadata={"model_requirements": requirement},
    )
    assert (
        CubeDocument.from_dict(document.to_dict()).metadata["model_requirements"]
        == requirement
    )
    legacy = CubeDocument("local/personal/Model.cube", "1.0.0")
    assert "model_requirements" not in CubeDocument.from_dict(legacy.to_dict()).metadata


@pytest.mark.parametrize(
    "requirement",
    [
        {"schemaVersion": True, "architecture": "SDXL", "family": ""},
        {"schemaVersion": 1, "architecture": 1, "family": ""},
        {"schemaVersion": 1, "architecture": "x" * 129, "family": ""},
        {"schemaVersion": 1, "architecture": "SDXL", "family": "", "path": "local"},
    ],
)
def test_cube_model_requirement_rejects_untrusted_data(requirement: object) -> None:
    """Reject unsupported schema, executable controls and machine-specific extra state."""
    document = CubeDocument(
        "local/personal/Model.cube",
        "1.0.0",
        metadata={"model_requirements": requirement},
    )
    with pytest.raises(CubeSchemaError):
        CubeDocument.from_dict(document.to_dict())
