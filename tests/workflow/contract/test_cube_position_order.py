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
"""Prove repeated free Cube permutations preserve native payloads and spacing."""

from __future__ import annotations

from copy import deepcopy
from typing import cast

import pytest

from sugarcubes.workflow_analysis import CubeGraphAnalysisService
from sugarcubes.workflow_cube_placement import cube_position_order
from sugarcubes.workflow_mutation import CubeGraphMutationService
from sugarcubes.workflow_mutation_errors import CubeGraphMutationError
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)


def test_repeated_arbitrary_permutations_cross_wiring_and_ordinary_boundaries() -> None:
    """Allow a start Cube last and an outputless Cube first without changing links."""

    ordinary = {
        "id": "boundary",
        "type": "Vendor",
        "pos": [900, 650],
        "size": [100, 100],
        "properties": {"opaque": True},
    }
    workflow = cube_workflow(
        {
            identity: image_passthrough_document(identity)
            for identity in ("start", "middle", "end")
        },
        loose_nodes=(ordinary,),
        links=(
            [1, 1, 0, "boundary", 0, "IMAGE"],
            [2, "boundary", 0, 2, 0, "IMAGE"],
            [3, 2, 0, 3, 0, "IMAGE"],
        ),
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes[:3]):
        node["pos"] = [index * 700, index * 15]
        node["size"] = [100 + index * 250, 200]
    nodes[0]["inputs"] = []
    nodes[2]["outputs"] = []
    original = deepcopy(workflow)
    service = CubeGraphMutationService(CubeGraphAnalysisService())
    current: object = workflow
    expected = ("start", "middle", "end")
    for requested in (
        ("end", "middle", "start"),
        ("middle", "start", "end"),
        ("start", "end", "middle"),
        ("start", "middle", "end"),
    ):
        result = service.reorder(
            current, segment_instance_ids=expected, ordered_instance_ids=requested
        )
        node_ids = {
            instance.instance_id: instance.node_id for instance in result.instances
        }
        assert cube_position_order(result.normalized_workflow, node_ids) == requested
        changed = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
        by_id = {str(node["id"]): node for node in changed}
        for index, identity in enumerate(requested[:-1]):
            left = by_id[node_ids[identity]]
            right = by_id[node_ids[requested[index + 1]]]
            assert (
                cast(list[float], right["pos"])[0]
                - cast(list[float], left["pos"])[0]
                - cast(list[float], left["size"])[0]
                >= 24
            )
        for before, after in zip(nodes, changed, strict=True):
            assert {key: value for key, value in after.items() if key != "pos"} == {
                key: value for key, value in before.items() if key != "pos"
            }
        assert changed[-1] == ordinary
        assert result.normalized_workflow["links"] == original["links"]
        assert result.normalized_workflow["definitions"] == original["definitions"]
        current = result.normalized_workflow
        expected = requested
    assert workflow == original


@pytest.mark.parametrize(
    "requested", [("first", "first"), ("first", "foreign"), ("first",)]
)
def test_rejects_incomplete_duplicate_or_foreign_permutations(
    requested: tuple[str, ...],
) -> None:
    """Reject malformed free ordering intent before returning a changed workflow."""

    workflow = cube_workflow(
        {
            identity: image_passthrough_document(identity)
            for identity in ("first", "second")
        }
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node["pos"] = [index * 344, 0]
        node["size"] = [320, 200]
    original = deepcopy(workflow)
    with pytest.raises(CubeGraphMutationError, match="exact Cube permutation"):
        CubeGraphMutationService(CubeGraphAnalysisService()).reorder(
            workflow,
            segment_instance_ids=("first", "second"),
            ordered_instance_ids=requested,
        )
    assert workflow == original
