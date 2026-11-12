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
"""Prove complete cart plans preserve native workflow contracts and canonical boundaries."""

from __future__ import annotations

from copy import deepcopy
from typing import cast

import pytest

from sugarcubes.cube_model import CubeDocument
from sugarcubes.workflow_analysis import CubeGraphAnalysisService
from sugarcubes.workflow_mutation import CubeGraphMutationService
from sugarcubes.workflow_mutation_errors import CubeGraphMutationError
from sugarcubes.workflow_stack_mutation import (
    CubeStackMutationService,
    ExistingCubeStackEntry,
    NewCubeStackEntry,
)
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)


def test_complete_cart_removes_adds_duplicates_and_reorders_without_mutating_source() -> (
    None
):
    """Keep existing definitions and ordinary records while planning repeated artifacts."""

    workflow = cube_workflow(
        {
            "first": image_passthrough_document("First"),
            "second": image_passthrough_document("Second"),
        }
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node["pos"] = [index * 344, 0]
        node["size"] = [320, 600]
    before = deepcopy(workflow)
    artifact = CubeDocument.from_dict(image_passthrough_document("First"))
    result = _service().apply(
        workflow,
        (
            NewCubeStackEntry("new-one", "First", artifact, (320.0, 600.0)),
            ExistingCubeStackEntry("first"),
            NewCubeStackEntry("new-two", "First", artifact, (480.0, 600.0)),
        ),
    )
    assert workflow == before
    assert tuple(
        value for segment in result.segments for value in segment.instance_ids
    ) == ("new-one", "first", "new-two")
    instances = {instance.instance_id: instance for instance in result.instances}
    assert instances["new-one"].instance_alias == "First 2"
    assert instances["new-two"].instance_alias == "First 3"
    assert instances["new-one"].cube_id == instances["new-two"].cube_id
    assert instances["first"].definition_id == "definition-first"
    nodes = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
    assert next(
        node for node in nodes if str(node["id"]) == instances["new-two"].node_id
    )["size"] == [480.0, 600.0]


def test_empty_cart_removes_only_cubes_and_keeps_ordinary_workflow_data() -> None:
    """Allow removing the complete Cube stack without replacing an ordinary graph."""

    ordinary = {
        "id": "ordinary",
        "type": "Opaque",
        "pos": [900, 600],
        "size": [120, 80],
        "properties": {"literal": "<b>keep</b>"},
    }
    workflow = cube_workflow(
        {"first": image_passthrough_document("First")}, loose_nodes=(ordinary,)
    )
    result = _service().apply(workflow, ())
    assert result.instances == ()
    assert result.normalized_workflow["nodes"] == [ordinary]


@pytest.mark.parametrize("ids", [("first", "first"), ("foreign",)])
def test_rejects_duplicate_or_unavailable_existing_instances(
    ids: tuple[str, ...],
) -> None:
    """Reject stale or duplicated references before producing any cart plan."""

    workflow = cube_workflow({"first": image_passthrough_document("First")})
    before = deepcopy(workflow)
    with pytest.raises(CubeGraphMutationError):
        _service().apply(
            workflow, tuple(ExistingCubeStackEntry(value) for value in ids)
        )
    assert workflow == before


def test_cart_reorders_explicitly_wired_cubes_without_changing_links() -> None:
    """Use the canonical reorder owner rather than granting cart-specific permissions."""

    workflow = cube_workflow(
        {
            "first": image_passthrough_document("First"),
            "second": image_passthrough_document("Second"),
        },
        links=((1, 1, 0, 2, 0, "IMAGE"),),
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node["pos"] = [index * 344, 0]
        node["size"] = [320, 200]
    before = deepcopy(workflow)
    result = _service().apply(
        workflow,
        (ExistingCubeStackEntry("second"), ExistingCubeStackEntry("first")),
    )
    nodes = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
    assert cast(list[float], nodes[1]["pos"])[0] < cast(list[float], nodes[0]["pos"])[0]
    assert result.normalized_workflow["links"] == before["links"]
    assert workflow == before


def test_rejects_invalid_frame_dimensions_and_allocates_suffixes_consistently() -> None:
    """Reject nonfinite geometry and keep repeated readable names deterministic."""

    workflow = cube_workflow({"first": image_passthrough_document("First")})
    artifact = CubeDocument.from_dict(image_passthrough_document("First"))
    with pytest.raises(CubeGraphMutationError, match="frame size"):
        _service().apply(
            workflow,
            (NewCubeStackEntry("new", "First", artifact, (float("nan"), 600.0)),),
        )


def _service() -> CubeStackMutationService:
    """Compose real canonical owners without substituting topology or geometry policy."""

    analysis = CubeGraphAnalysisService()
    return CubeStackMutationService(analysis, CubeGraphMutationService(analysis))
