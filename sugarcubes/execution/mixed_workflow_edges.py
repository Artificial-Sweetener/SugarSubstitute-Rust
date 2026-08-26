#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Rebind ordinary workflow links that cross Cube and loose-node boundaries."""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping, Sequence

from ..workflow import CanonicalWorkflow, CubeInstance
from .boundary_lowering import wire_boundary_target
from .loose_node_lowering import root_link_sources
from .lowering_values import connection
from .models import BoundaryBinding, CubeTopology, ExecutionNodeOwner

_INACTIVE_MODES = frozenset({2, 4})


class MixedWorkflowEdgeLowerer:
    """Preserve ordinary dataflow while keeping loose nodes semantically opaque."""

    def lower(
        self,
        workflow: CanonicalWorkflow,
        prompt: MutableMapping[str, dict[str, object]],
        node_owners: Mapping[str, ExecutionNodeOwner],
        bindings: tuple[BoundaryBinding, ...],
        topology: CubeTopology,
    ) -> None:
        """Replace root Cube endpoints only on Cube-to-loose crossing links."""

        instances_by_node = {
            instance.node_id: instance for instance in workflow.instances
        }
        output_bindings = _output_bindings(workflow.instances, bindings)
        self._wire_cube_sources(
            prompt,
            node_owners=node_owners,
            instances_by_node=instances_by_node,
            output_bindings=output_bindings,
            topology=topology,
        )
        self._wire_cube_targets(
            workflow,
            prompt,
            instances_by_node=instances_by_node,
            bindings=bindings,
            topology=topology,
        )

    def _wire_cube_sources(
        self,
        prompt: MutableMapping[str, dict[str, object]],
        *,
        node_owners: Mapping[str, ExecutionNodeOwner],
        instances_by_node: Mapping[str, CubeInstance],
        output_bindings: Mapping[tuple[str, int], BoundaryBinding],
        topology: CubeTopology,
    ) -> None:
        """Rebind loose inputs whose serialized source is a Cube wrapper."""

        for node_id, node in prompt.items():
            owner = node_owners.get(node_id)
            if owner is None or not owner.is_loose:
                continue
            inputs = node.get("inputs")
            if not isinstance(inputs, MutableMapping):
                continue
            for name, value in tuple(inputs.items()):
                source = connection(value)
                if source is None or source[0] not in instances_by_node:
                    continue
                instance = instances_by_node[source[0]]
                if (
                    topology.instances[instance.instance_id].execution_mode
                    in _INACTIVE_MODES
                ):
                    continue
                binding = output_bindings.get((instance.node_id, source[1]))
                if binding is not None and binding.output_index is not None:
                    inputs[name] = [binding.node_id, binding.output_index]

    def _wire_cube_targets(
        self,
        workflow: CanonicalWorkflow,
        prompt: MutableMapping[str, dict[str, object]],
        *,
        instances_by_node: Mapping[str, CubeInstance],
        bindings: tuple[BoundaryBinding, ...],
        topology: CubeTopology,
    ) -> None:
        """Wire Cube public inputs whose serialized source is a loose node."""

        sources = root_link_sources(workflow.payload)
        input_bindings = _input_bindings(bindings)
        for instance in workflow.instances:
            if (
                topology.instances[instance.instance_id].execution_mode
                in _INACTIVE_MODES
            ):
                continue
            for entry in _boundary_entries(instance.payload, "inputs"):
                link_id = entry.get("link")
                source = sources.get(link_id)
                name = entry.get("name")
                if (
                    source is None
                    or source[0] in instances_by_node
                    or not isinstance(name, str)
                ):
                    continue
                source_binding = BoundaryBinding(
                    instance_id="",
                    binding="",
                    direction="output",
                    node_id=source[0],
                    output_index=source[1],
                )
                for target in input_bindings.get((instance.instance_id, name), ()):
                    wire_boundary_target(prompt, source_binding, target)


def _output_bindings(
    instances: Sequence[CubeInstance], bindings: tuple[BoundaryBinding, ...]
) -> dict[tuple[str, int], BoundaryBinding]:
    """Index live Cube output endpoints by serialized root node and slot."""

    by_name = {
        (binding.instance_id, binding.binding): binding
        for binding in bindings
        if binding.direction == "output"
    }
    result: dict[tuple[str, int], BoundaryBinding] = {}
    for instance in instances:
        for slot, entry in enumerate(_boundary_entries(instance.payload, "outputs")):
            name = entry.get("name")
            if isinstance(name, str):
                binding = by_name.get((instance.instance_id, name))
                if binding is not None:
                    result[(instance.node_id, slot)] = binding
    return result


def _input_bindings(
    bindings: tuple[BoundaryBinding, ...],
) -> dict[tuple[str, str], tuple[BoundaryBinding, ...]]:
    """Group public Cube input targets without collapsing native fanout."""

    grouped: dict[tuple[str, str], list[BoundaryBinding]] = {}
    for binding in bindings:
        if binding.direction == "input":
            grouped.setdefault((binding.instance_id, binding.binding), []).append(
                binding
            )
    return {key: tuple(value) for key, value in grouped.items()}


def _boundary_entries(
    payload: Mapping[str, object], direction: str
) -> tuple[Mapping[str, object], ...]:
    """Return only mapping-valued serialized boundary entries."""

    values = payload.get(direction)
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        return ()
    return tuple(value for value in values if isinstance(value, Mapping))
