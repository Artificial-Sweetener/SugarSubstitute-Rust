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
"""Prove wild Cube classification never changes embedded execution truth."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from sugarcubes.library import CatalogCubeArtifact, CubeLibraryClassService
from sugarcubes.library.stable_repository import StableCubeRepository
from sugarcubes.workflow import read_canonical_workflow
from tests.workflow.support.workflow_fixtures import cube_workflow


def test_offline_definition_is_unsaved_wild_and_remains_read_only(
    tmp_path: Path,
) -> None:
    """Classify valid embedded content without requiring any installed catalog."""

    workflow = read_canonical_workflow(cube_workflow())
    service = CubeLibraryClassService(stable=StableCubeRepository(tmp_path / "stable"))

    classification = service.classify_workflow(workflow).definitions[0]

    assert classification.primary_class == "none"
    assert classification.access == "read_only"
    assert classification.instance_ids == ("cube-sdxl-text-1",)
    assert classification.matches == ()
    assert "save_to_stable" in classification.permitted_operations
    assert "edit_definition" not in classification.permitted_operations


def test_reports_exact_and_divergent_matches_without_shadowing_workflow(
    tmp_path: Path,
) -> None:
    """Retain all machine relationships while the embedded payload stays unchanged."""

    source = cube_workflow()
    workflow = read_canonical_workflow(source)
    definition = workflow.definitions[0]
    candidates = (
        CatalogCubeArtifact(
            definition.cube_id,
            definition.cube_version,
            definition.semantic_hash,
            "local",
            "writable",
            "local:personal",
        ),
        CatalogCubeArtifact(
            definition.cube_id,
            definition.cube_version,
            "f" * 64,
            "synced",
            "read_only",
            "github:external/pack",
        ),
    )
    service = CubeLibraryClassService(
        stable=StableCubeRepository(tmp_path / "stable"),
        catalog_artifacts=lambda: candidates,
    )

    classification = service.classify_workflow(workflow).definitions[0]

    assert classification.primary_class == "local"
    assert classification.access == "writable"
    assert [match.state for match in classification.matches] == ["exact"]
    assert [match.state for match in classification.divergent_matches] == ["divergent"]
    assert workflow.payload == source
    assert workflow.payload is not source


def test_writable_source_authorizes_editing_when_embedded_content_has_diverged(
    tmp_path: Path,
) -> None:
    """Keep source ownership independent from exact semantic equality."""

    workflow = read_canonical_workflow(cube_workflow())
    definition = workflow.definitions[0]
    service = CubeLibraryClassService(
        stable=StableCubeRepository(tmp_path / "stable"),
        catalog_artifacts=lambda: (
            CatalogCubeArtifact(
                definition.cube_id,
                definition.cube_version,
                "f" * 64,
                "local",
                "writable",
                "local:personal",
            ),
        ),
    )

    classification = service.classify_workflow(workflow).definitions[0]

    assert classification.matches == ()
    assert [match.state for match in classification.divergent_matches] == ["divergent"]
    assert classification.access == "writable"
    assert "edit_definition" in classification.permitted_operations


def test_duplicate_instances_share_definition_classification(tmp_path: Path) -> None:
    """Keep one content classification while preserving stable instance identities."""

    source = cube_workflow()
    nodes = source["nodes"]
    assert isinstance(nodes, list)
    duplicate = deepcopy(nodes[0])
    assert isinstance(duplicate, dict)
    properties = duplicate["properties"]
    assert isinstance(properties, dict)
    identity = properties["sugarcubes_cube"]
    assert isinstance(identity, dict)
    identity["instance_id"] = "cube-sdxl-text-2"
    duplicate["id"] = 8
    nodes.append(duplicate)
    workflow = read_canonical_workflow(source)
    service = CubeLibraryClassService(stable=StableCubeRepository(tmp_path / "stable"))

    classification = service.classify_workflow(workflow).definitions[0]

    assert classification.instance_ids == ("cube-sdxl-text-1", "cube-sdxl-text-2")
