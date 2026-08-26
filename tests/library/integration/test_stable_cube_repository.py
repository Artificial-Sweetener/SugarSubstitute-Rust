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
"""Verify content-addressed Stable persistence and collision safety."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from sugarcubes.library import (
    CubeLibraryClassService,
    StableCubeSaveRequest,
    StableCubeRepository,
)
from sugarcubes.workflow import read_canonical_workflow
from tests.workflow.support.workflow_fixtures import cube_workflow


def test_stable_save_is_idempotent_and_preserves_exact_payload(tmp_path: Path) -> None:
    """Store one exact object without identity changes, lineage, or graph rebinds."""

    workflow = read_canonical_workflow(cube_workflow())
    definition = workflow.definitions[0]
    service = CubeLibraryClassService(stable=StableCubeRepository(tmp_path / "stable"))
    request = StableCubeSaveRequest(definition.definition_id, definition.semantic_hash)

    first = service.save_to_stable(workflow, request)
    second = service.save_to_stable(workflow, request)

    assert first.created is True
    assert second.created is False
    assert first.cube_id == definition.cube_id
    assert first.semantic_hash == definition.semantic_hash
    assert first.classification.primary_class == "stable"
    assert first.classification.access == "read_only"
    object_path = tmp_path / "stable" / "objects" / f"{definition.semantic_hash}.json"
    assert json.loads(object_path.read_text(encoding="utf-8")) == definition.payload


def test_same_identity_version_with_different_content_cannot_overwrite(
    tmp_path: Path,
) -> None:
    """Keep both conflicting claims under independent content addresses."""

    first_workflow = read_canonical_workflow(cube_workflow())
    changed_source = deepcopy(cube_workflow())
    definitions = changed_source["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    definition = subgraphs[0]
    assert isinstance(definition, dict)
    definition["nodes"] = [{"id": 1, "type": "DifferentLoader"}]
    extra = definition["extra"]
    assert isinstance(extra, dict)
    document = extra["sugarcubes_document"]
    assert isinstance(document, dict)
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    implementation["nodes"] = {
        "checkpoint": {"class_type": "DifferentLoader", "inputs": {}}
    }
    second_workflow = read_canonical_workflow(changed_source)
    repository = StableCubeRepository(tmp_path / "stable")
    service = CubeLibraryClassService(stable=repository)

    for workflow in (first_workflow, second_workflow):
        embedded = workflow.definitions[0]
        service.save_to_stable(
            workflow,
            StableCubeSaveRequest(embedded.definition_id, embedded.semantic_hash),
        )

    assert len(repository.list_artifacts()) == 2
    assert len(tuple((tmp_path / "stable" / "objects").glob("*.json"))) == 2


def test_stale_expected_hash_fails_before_writing(tmp_path: Path) -> None:
    """Reject stale UI state without creating any Stable directories."""

    workflow = read_canonical_workflow(cube_workflow())
    service = CubeLibraryClassService(stable=StableCubeRepository(tmp_path / "stable"))

    with pytest.raises(ValueError, match="changed after classification"):
        service.save_to_stable(
            workflow,
            StableCubeSaveRequest(workflow.definitions[0].definition_id, "0" * 64),
        )

    assert not (tmp_path / "stable").exists()
