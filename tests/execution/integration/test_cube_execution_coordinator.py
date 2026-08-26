#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify the Phase-3 execution coordinator fails closed and remains immutable."""

from __future__ import annotations

from copy import deepcopy
from collections.abc import Mapping

import pytest

from sugarcubes.execution import (
    CubeBoundaryEndpoint,
    CubeExecutionCoordinator,
    CubeExecutionRequest,
    CubeOptimizationOptions,
    CubeOptimizationReport,
    CubePromptOptimizer,
    CubeTopology,
    NodeOwner,
    CubeTopologyError,
    ProximityConnection,
    QueueMetadata,
)
from sugarcubes.execution.models import ApiPrompt, NodeDefinitions
from tests.execution.support.execution_fixtures import (
    consumer_document,
    cube_workflow,
    image_passthrough_document,
    provider_document,
)


def test_prepare_returns_complete_report_and_never_mutates_saved_workflow() -> None:
    """Apply execution-only topology, lowering, and inheritance to a deep copy."""

    workflow = cube_workflow(
        {"cube-a": provider_document(), "cube-b": consumer_document()}
    )
    original = deepcopy(workflow)
    request = CubeExecutionRequest(
        workflow=workflow,
        proximity_connections=(
            ProximityConnection(
                source=CubeBoundaryEndpoint("cube-a", "output.resource"),
                target=CubeBoundaryEndpoint("cube-b", "input.resource"),
            ),
        ),
        queue=QueueMetadata(partial_execution_targets=("cube-b",)),
    )

    prepared = CubeExecutionCoordinator().prepare(request)

    assert workflow == original
    expected_partial_targets = tuple(
        identity.execution_id
        for identity in prepared.report.output_identities
        if identity.instance_id == "cube-b"
    )
    assert prepared.queue.partial_execution_targets == expected_partial_targets
    assert prepared.report.phase_order == (
        "workflow",
        "topology",
        "lowering",
        "inheritance",
        "optimization",
        "instrumentation",
    )
    assert prepared.report.execution_owner == "sugarcubes"
    assert prepared.report.topology_edges[0].source_instance_id == "cube-a"
    assert len(prepared.report.inherited_bindings) == 3
    assert all(owner.instance_id for owner in prepared.node_owners.values())
    assert len(prepared.report.output_identities) == 2


def test_prepare_instruments_native_outputs_without_a_portable_document() -> None:
    """Keep legacy native-only Cubes queueable through their saved host surface."""

    workflow = cube_workflow({"cube-a": image_passthrough_document("Native")})
    definitions = workflow["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    native = subgraphs[0]
    assert isinstance(native, dict)
    native.update(
        {
            "inputs": [],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 7,
                    "type": "ImagePass",
                    "inputs": [{"name": "image", "widget": {"name": "image"}}],
                    "outputs": [{"name": "image", "links": [1]}],
                    "properties": {"sugarcubes_symbol": "image"},
                }
            ],
            "links": [
                {
                    "id": 1,
                    "origin_id": 7,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                }
            ],
        }
    )
    extra = native["extra"]
    assert isinstance(extra, dict)
    document = extra.pop("sugarcubes_document")
    assert isinstance(document, dict)
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    metadata = extra["sugarcubes_cube"]
    assert isinstance(metadata, dict)
    metadata["definitions"] = implementation["definitions"]
    root_nodes = workflow["nodes"]
    assert isinstance(root_nodes, list)
    root = root_nodes[0]
    assert isinstance(root, dict)
    root["inputs"] = []
    root["outputs"] = [{"name": "image", "type": "IMAGE"}]

    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )

    assert [
        (item.binding, item.output_slot) for item in prepared.report.output_identities
    ] == [("image", 0)]
    assert prepared.prompt["__sugarcubes_cube_output__:1:0"]["class_type"] == (
        "SugarCubes.CubeOutput"
    )


def test_prepare_reports_each_phase_duration_from_one_monotonic_clock() -> None:
    """Measure every owned phase without placing timing in semantic identity."""

    readings = iter(index * 1_000_000 for index in range(13))
    coordinator = CubeExecutionCoordinator(clock_ns=lambda: next(readings))

    prepared = coordinator.prepare(
        CubeExecutionRequest(workflow=cube_workflow({"cube-a": provider_document()}))
    )

    assert prepared.report.phase_timings_ms == (
        ("workflow", 1.0),
        ("topology", 1.0),
        ("lowering", 1.0),
        ("inheritance", 1.0),
        ("optimization", 1.0),
        ("instrumentation", 1.0),
    )


def test_prepare_stops_before_lowering_when_topology_is_invalid() -> None:
    """Make a topology failure observable and leave nothing queueable."""

    workflow = cube_workflow(
        {
            "cube-a": image_passthrough_document("A"),
            "cube-b": image_passthrough_document("B"),
        },
        links=[[1, 1, 0, 2, 0, "IMAGE"], [2, 2, 0, 1, 0, "IMAGE"]],
    )

    with pytest.raises(CubeTopologyError):
        CubeExecutionCoordinator().prepare(CubeExecutionRequest(workflow=workflow))


def test_prepare_is_independent_of_root_node_and_definition_array_order() -> None:
    """Use stable Cube and symbol identities instead of Comfy numeric ordering."""

    workflow = cube_workflow(
        {"cube-a": provider_document(), "cube-b": consumer_document()}
    )
    reordered = deepcopy(workflow)
    nodes = reordered["nodes"]
    assert isinstance(nodes, list)
    reordered["nodes"] = list(reversed(nodes))
    definitions = reordered["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    definitions["subgraphs"] = list(reversed(subgraphs))
    connection = ProximityConnection(
        source=CubeBoundaryEndpoint("cube-a", "output.resource"),
        target=CubeBoundaryEndpoint("cube-b", "input.resource"),
    )
    coordinator = CubeExecutionCoordinator()

    first = coordinator.prepare(
        CubeExecutionRequest(workflow=workflow, proximity_connections=(connection,))
    )
    second = coordinator.prepare(
        CubeExecutionRequest(workflow=reordered, proximity_connections=(connection,))
    )

    assert first.prompt == second.prompt
    assert first.node_owners == second.node_owners
    assert first.report.topology_edges == second.report.topology_edges
    assert first.report.inherited_bindings == second.report.inherited_bindings


def test_prepare_fails_open_to_the_inherited_prompt_when_optimizer_throws() -> None:
    """Keep required transforms fail-closed while making optimization non-fatal."""

    workflow = cube_workflow({"cube-a": provider_document()})
    baseline = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(
            workflow=workflow,
            optimization=CubeOptimizationOptions(enabled=False),
        )
    )

    prepared = CubeExecutionCoordinator(optimizer=_FailingOptimizer()).prepare(
        CubeExecutionRequest(workflow=workflow)
    )

    assert prepared.prompt == baseline.prompt
    assert prepared.report.optimization == CubeOptimizationReport.failed_open(
        len(baseline.prompt) - len(baseline.report.output_identities), "RuntimeError"
    )
    assert [diagnostic.code for diagnostic in prepared.report.diagnostics] == [
        "execution.optimization.failed_open"
    ]


class _FailingOptimizer(CubePromptOptimizer):
    """Inject one optimizer failure at the application boundary."""

    def optimize(
        self,
        prompt: ApiPrompt,
        *,
        node_owners: Mapping[str, NodeOwner],
        node_definitions: NodeDefinitions,
        topology: CubeTopology,
        options: CubeOptimizationOptions,
        protected_node_ids: frozenset[str] = frozenset(),
    ) -> tuple[ApiPrompt, CubeOptimizationReport]:
        """Raise after all required preparation phases have succeeded."""

        _ = prompt, node_owners, node_definitions, topology, options, protected_node_ids
        raise RuntimeError("injected optimizer failure")
