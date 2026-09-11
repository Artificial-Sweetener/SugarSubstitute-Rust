#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Prove headless clients execute the same serialized Cube proximity graph."""

from __future__ import annotations

from typing import cast

from sugarcubes.execution import CubeExecutionCoordinator, CubeExecutionRequest
from tests.execution.support.execution_fixtures import (
    consumer_document,
    cube_workflow,
    provider_document,
)


def test_prepare_derives_proximity_and_resource_inheritance_without_frontend() -> None:
    """Let SugarCubes own topology when a headless client submits only a graph."""

    workflow = cube_workflow(
        {"provider": provider_document(), "consumer": consumer_document()}
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[0].update({"pos": [0, 0], "size": [100, 100]})
    nodes[1].update({"pos": [220, 0], "size": [100, 100]})

    prepared = CubeExecutionCoordinator().prepare(CubeExecutionRequest(workflow))

    assert [
        (edge.source_instance_id, edge.target_instance_id, edge.origin.value)
        for edge in prepared.report.topology_edges
    ] == [("provider", "consumer", "proximity")]
    assert {
        (binding.target_instance_id, binding.slot)
        for binding in prepared.report.inherited_bindings
    } == {("consumer", "model"), ("consumer", "clip"), ("consumer", "vae")}
