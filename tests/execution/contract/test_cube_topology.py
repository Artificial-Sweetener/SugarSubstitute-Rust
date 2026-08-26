#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Specify deterministic Cube-only topology independently of presentation."""

from __future__ import annotations

import pytest

from sugarcubes.execution import (
    ConnectionOrigin,
    CubeBoundaryConnection,
    CubeBoundaryEndpoint,
    CubeTopologyError,
    ProximityConnection,
    build_cube_topology,
)
from sugarcubes.execution.limits import MAX_PROXIMITY_CONNECTIONS
from sugarcubes.workflow import read_canonical_workflow
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)


def test_topology_collapses_duplicate_observations_and_ignores_loose_bridges() -> None:
    """Keep only direct Cube edges and stable instance identities."""

    workflow = cube_workflow(
        {
            "cube-b": image_passthrough_document("B"),
            "cube-a": image_passthrough_document("A"),
            "cube-c": image_passthrough_document("C"),
        },
        links=[
            [1, 2, 0, 1, 0, "IMAGE"],
            [2, 1, 0, 99, 0, "IMAGE"],
            [3, 99, 0, 3, 0, "IMAGE"],
        ],
        loose_nodes=[
            {
                "id": 99,
                "type": "ImagePass",
                "inputs": [{"name": "image", "link": 2}],
                "outputs": [{"name": "image", "links": [3]}],
                "properties": {},
            }
        ],
    )
    proximity = ProximityConnection(
        source=CubeBoundaryEndpoint("cube-a", "output.image"),
        target=CubeBoundaryEndpoint("cube-b", "input.image"),
    )

    topology = build_cube_topology(read_canonical_workflow(workflow), (proximity,))

    assert tuple(sorted(topology.instances)) == ("cube-a", "cube-b", "cube-c")
    assert topology.edges == (
        CubeBoundaryConnection(
            source=CubeBoundaryEndpoint("cube-a", "output.image"),
            target=CubeBoundaryEndpoint("cube-b", "input.image"),
            origin=ConnectionOrigin.EXPLICIT,
        ),
    )
    assert topology.components == (("cube-a", "cube-b"), ("cube-c",))
    assert topology.topological_order == ("cube-a", "cube-b", "cube-c")


def test_cycle_is_a_located_fail_closed_topology_error() -> None:
    """Reject cycles rather than selecting an incidental native node order."""

    workflow = cube_workflow(
        {
            "cube-a": image_passthrough_document("A"),
            "cube-b": image_passthrough_document("B"),
        },
        links=[[1, 1, 0, 2, 0, "IMAGE"], [2, 2, 0, 1, 0, "IMAGE"]],
    )

    with pytest.raises(CubeTopologyError) as captured:
        build_cube_topology(read_canonical_workflow(workflow))

    assert captured.value.code == "execution.topology.cycle"
    assert captured.value.instance_ids == ("cube-a", "cube-b")


def test_proximity_connections_are_bounded_before_topology_traversal() -> None:
    """Reject oversized host observations before iterating their endpoints."""

    workflow = cube_workflow({"cube-a": image_passthrough_document("A")})
    connection = ProximityConnection(
        source=CubeBoundaryEndpoint("cube-a", "output.image"),
        target=CubeBoundaryEndpoint("cube-a", "input.image"),
    )

    with pytest.raises(CubeTopologyError) as captured:
        build_cube_topology(
            read_canonical_workflow(workflow),
            (connection,) * (MAX_PROXIMITY_CONNECTIONS + 1),
        )

    assert captured.value.code == "execution.topology.proximity_limit_exceeded"
    assert captured.value.instance_ids == ()
