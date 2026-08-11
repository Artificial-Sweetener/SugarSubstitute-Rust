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
"""Tests for persisted cube identity projection invariants."""

from __future__ import annotations

from pathlib import Path

import json
from typing import Any

import pytest

from sugarcubes.cube_model import (
    CubeSchemaError,
    apply_cube_identity_projection,
    build_cube_definition_key,
    iter_cube_identity_projection_violations,
)
from sugarcubes.exporter import ExportedCube, write_cube_to_path

CANONICAL_CUBE_ID = "Artificial-Sweetener/Base-Cubes/Anima/Diffusion Upscale.cube"
OTHER_CUBE_ID = "Artificial-Sweetener/Base-Cubes/SDXL/Diffusion Upscale.cube"


def _payload_with_layout(*, cube_id: str = CANONICAL_CUBE_ID) -> dict[str, Any]:
    """Build one compact payload with stale embedded identity metadata."""

    return {
        "cube_id": cube_id,
        "version": "3.0.0",
        "implementation": {
            "layout": {
                "groups": [
                    {
                        "title": "Anima/Diffusion Upscale",
                        "sugarcubes": {
                            "schema": 6,
                            "managed": True,
                            "cube_id": cube_id,
                            "default_alias": "Anima/Diffusion Upscale",
                            "target_model": "Anima",
                            "cube_version": "2.0.0",
                            "cube_revision_ref": "WORKTREE",
                            "cube_definition_key": f"{cube_id}@2.0.0",
                            "surface_signature": "surface-a",
                            "markers": {"inputs": ["1"], "outputs": ["3"]},
                            "nodes": ["2"],
                            "bounds": {"x": 0, "y": 0, "w": 640, "h": 200},
                        },
                    },
                    {
                        "title": "Other cube",
                        "sugarcubes": {
                            "cube_id": OTHER_CUBE_ID,
                            "cube_version": "9.9.9",
                            "cube_definition_key": f"{OTHER_CUBE_ID}@9.9.9",
                        },
                    },
                ],
                "markers": {
                    "input.value": {
                        "id": "1",
                        "properties": {"sugarcubes_cube_version": "2.0.0"},
                    }
                },
            }
        },
    }


def test_identity_projection_rewrites_same_cube_embedded_identity() -> None:
    """Project top-level cube identity into same-cube layout chrome."""

    payload = _payload_with_layout()

    apply_cube_identity_projection(payload)

    group_metadata = payload["implementation"]["layout"]["groups"][0]["sugarcubes"]
    other_metadata = payload["implementation"]["layout"]["groups"][1]["sugarcubes"]
    marker_properties = payload["implementation"]["layout"]["markers"]["input.value"][
        "properties"
    ]
    assert group_metadata["cube_id"] == CANONICAL_CUBE_ID
    assert group_metadata["cube_version"] == "3.0.0"
    assert group_metadata["cube_definition_key"] == (f"{CANONICAL_CUBE_ID}@3.0.0")
    assert group_metadata["surface_signature"] == "surface-a"
    assert group_metadata["bounds"] == {"x": 0, "y": 0, "w": 640, "h": 200}
    assert other_metadata["cube_version"] == "9.9.9"
    assert marker_properties["sugarcubes_cube_version"] == "3.0.0"
    assert iter_cube_identity_projection_violations(payload) == ()


def test_identity_projection_rewrites_structured_definition_metadata() -> None:
    """Handle structured group metadata without depending on frontend flattening."""

    payload: dict[str, Any] = {
        "cube_id": CANONICAL_CUBE_ID,
        "version": "2.0.0",
        "implementation": {
            "layout": {
                "groups": [
                    {
                        "sugarcubes": {
                            "definition": {
                                "cube_id": CANONICAL_CUBE_ID,
                                "cube_version": "1.0.0",
                                "cube_definition_key": f"{CANONICAL_CUBE_ID}@1.0.0",
                            },
                            "instance": {"nodes": ["2"]},
                        }
                    }
                ]
            }
        },
    }

    apply_cube_identity_projection(payload)

    metadata = payload["implementation"]["layout"]["groups"][0]["sugarcubes"]
    definition = metadata["definition"]
    assert metadata["cube_version"] == "2.0.0"
    assert definition["cube_version"] == "2.0.0"
    assert definition["cube_definition_key"] == f"{CANONICAL_CUBE_ID}@2.0.0"


def test_identity_projection_reports_mismatches_for_audit() -> None:
    """Expose a reusable consistency audit for persisted cube payloads."""

    payload = _payload_with_layout()

    assert iter_cube_identity_projection_violations(payload) == (
        "group[0].cube_version must be 3.0.0",
        f"group[0].cube_definition_key must be {CANONICAL_CUBE_ID}@3.0.0",
        "marker[0].sugarcubes_cube_version must be 3.0.0",
    )


def test_identity_projection_rejects_missing_authoritative_identity() -> None:
    """Fail closed when the authoritative document identity is unavailable."""

    with pytest.raises(CubeSchemaError, match="cube_id"):
        apply_cube_identity_projection({"version": "1.0.0"})

    payload = _payload_with_layout()
    payload["version"] = ""
    with pytest.raises(CubeSchemaError, match="version"):
        apply_cube_identity_projection(payload)


def test_identity_projection_leaves_versionless_bare_payloads_unchanged() -> None:
    """Avoid turning minimal write-helper payloads into full schema documents."""

    payload = {"cube_id": CANONICAL_CUBE_ID, "metadata": {}}

    apply_cube_identity_projection(payload)

    assert payload == {"cube_id": CANONICAL_CUBE_ID, "metadata": {}}


def test_write_cube_to_path_persists_projected_identity(tmp_path: Path) -> None:
    """Normalize embedded identity at the exporter file write boundary."""

    target_path = tmp_path / "Diffusion Upscale.cube"
    exported = ExportedCube(
        default_alias="Anima/Diffusion Upscale",
        cube=_payload_with_layout(),
        warnings=[],
    )

    write_cube_to_path(exported, target_path)

    stored = json.loads(target_path.read_text(encoding="utf-8"))
    group_metadata = stored["implementation"]["layout"]["groups"][0]["sugarcubes"]
    assert group_metadata["cube_version"] == "3.0.0"
    assert group_metadata["cube_definition_key"] == build_cube_definition_key(
        CANONICAL_CUBE_ID,
        "3.0.0",
    )
    assert iter_cube_identity_projection_violations(stored) == ()
