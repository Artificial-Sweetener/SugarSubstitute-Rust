#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Coordinate native Cube, loose-node, and boundary lowering owners."""

from __future__ import annotations

from typing import Mapping

from ..cube_model import CubeDocument, CubeSchemaError
from ..workflow import CanonicalWorkflow
from .boundary_lowering import apply_topology_edges
from .cube_document_lowering import CubeDocumentLowerer
from .errors import CubeLoweringError
from .loose_node_lowering import LooseWorkflowNodeLowerer
from .mixed_workflow_edges import MixedWorkflowEdgeLowerer
from .models import BoundaryBinding, CubeTopology, LoweringResult, NodeOwner


class NativeCubeWorkflowLowerer:
    """Own the complete lowering transaction while delegating focused mechanics."""

    def __init__(
        self,
        *,
        cubes: CubeDocumentLowerer | None = None,
        loose_nodes: LooseWorkflowNodeLowerer | None = None,
        mixed_edges: MixedWorkflowEdgeLowerer | None = None,
    ) -> None:
        """Bind cohesive Cube and loose graph projection owners."""

        self._cubes = cubes or CubeDocumentLowerer()
        self._loose_nodes = loose_nodes or LooseWorkflowNodeLowerer()
        self._mixed_edges = mixed_edges or MixedWorkflowEdgeLowerer()

    def lower(
        self, workflow: CanonicalWorkflow, topology: CubeTopology
    ) -> LoweringResult:
        """Create one derived prompt without mutating canonical workflow state."""

        prompt: dict[str, dict[str, object]] = {}
        owners: dict[str, NodeOwner] = {}
        bindings: list[BoundaryBinding] = []
        definitions: dict[str, Mapping[str, object]] = {}
        definition_index = workflow.definition_index()
        for instance_id in topology.topological_order:
            instance = topology.instances[instance_id]
            embedded = definition_index[instance.definition_id]
            document = _read_optional_document(
                instance_id, instance.definition_id, embedded.document
            )
            if instance.execution_mode in {2, 4}:
                bindings.extend(
                    self._cubes.describe_bindings(
                        instance_id,
                        document,
                        embedded.payload,
                        instance.payload,
                    )
                )
                continue
            lowered_cube = self._cubes.lower(
                instance_id,
                document,
                embedded.payload,
                instance.payload,
                _connected_input_bindings(workflow, topology, instance_id),
                embedded.native_subgraphs,
                embedded.native_node_definitions,
            )
            for node_id, node in lowered_cube.prompt.items():
                if node_id in prompt:
                    raise CubeLoweringError(
                        "execution.lowering.duplicate_node",
                        f"Lowered execution node id '{node_id}' is duplicated.",
                    )
                prompt[node_id] = node
                owners[node_id] = NodeOwner(instance_id)
            definitions.update(lowered_cube.node_definitions)
            bindings.extend(lowered_cube.boundary_bindings)
        self._loose_nodes.lower(workflow, prompt, owners, definitions)
        apply_topology_edges(prompt, tuple(bindings), topology)
        self._mixed_edges.lower(workflow, prompt, owners, tuple(bindings), topology)
        return LoweringResult(
            prompt=prompt,
            node_owners=owners,
            boundary_bindings=tuple(sorted(bindings, key=_binding_key)),
            node_definitions=definitions,
        )


def _read_optional_document(
    instance_id: str,
    definition_id: str,
    payload: Mapping[str, object] | None,
) -> CubeDocument | None:
    """Read optional portable semantics without blocking saved native execution."""

    if payload is None:
        return None
    try:
        return CubeDocument.from_dict(payload)
    except CubeSchemaError as exc:
        raise CubeLoweringError(
            "execution.lowering.invalid_cube_document",
            f"Cube instance '{instance_id}' cannot lower: {exc}",
            path=f"$.definitions.{definition_id}",
        ) from exc


def _binding_key(binding: BoundaryBinding) -> tuple[str, str, str, str, str, int]:
    """Sort public bindings independently of mutable numeric workflow order."""

    return (
        binding.instance_id,
        binding.binding,
        binding.direction,
        binding.node_id,
        binding.input_name or "",
        binding.output_index if binding.output_index is not None else -1,
    )


def _connected_input_bindings(
    workflow: CanonicalWorkflow, topology: CubeTopology, instance_id: str
) -> frozenset[str]:
    """Return host boundaries participating in ordinary or proximity links."""

    connected = {
        edge.target_binding
        for edge in topology.edges
        if edge.target_instance_id == instance_id
    }
    instance = topology.instances[instance_id]
    inputs = instance.payload.get("inputs")
    if isinstance(inputs, list):
        connected.update(
            name
            for entry in inputs
            if isinstance(entry, Mapping) and entry.get("link") is not None
            if isinstance((name := entry.get("name")), str) and name
        )
    return frozenset(connected)
