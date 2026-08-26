#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Inject stable prompt-only sinks for every exported Cube output."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass

from ..workflow import CanonicalWorkflow
from .errors import CubeInstrumentationError
from .models import (
    ApiPrompt,
    BoundaryBinding,
    CubeOutputIdentity,
    CubeTopology,
    ExecutionNodeOwner,
)

_EXECUTION_PREFIX = "__sugarcubes_cube_output__"


@dataclass(frozen=True)
class InstrumentedPrompt:
    """Carry a prompt, complete ownership, and stable output identities."""

    prompt: ApiPrompt
    node_owners: Mapping[str, ExecutionNodeOwner]
    output_identities: tuple[CubeOutputIdentity, ...]


class CubeOutputInstrumenter:
    """Own output sink construction independently of presentation code."""

    def instrument(
        self,
        prompt: ApiPrompt,
        *,
        node_owners: Mapping[str, ExecutionNodeOwner],
        boundary_bindings: tuple[BoundaryBinding, ...],
        workflow: CanonicalWorkflow,
        topology: CubeTopology,
    ) -> InstrumentedPrompt:
        """Add one non-persisted sink for each active public Cube output."""

        result = deepcopy(prompt)
        owners = dict(node_owners)
        output_bindings = {
            (binding.instance_id, binding.binding): binding
            for binding in boundary_bindings
            if binding.direction == "output"
        }
        definitions = workflow.definition_index()
        identities: list[CubeOutputIdentity] = []
        for instance_id in topology.topological_order:
            instance = topology.instances[instance_id]
            if instance.execution_mode in {2, 4}:
                continue
            embedded = definitions[instance.definition_id]
            output_names = _document_output_names(
                embedded.document
            ) or _native_output_names(instance.payload)
            for output_slot, binding_name in enumerate(output_names):
                binding = output_bindings.get((instance_id, binding_name))
                if binding is None or binding.output_index is None:
                    raise CubeInstrumentationError(
                        "execution.instrumentation.missing_output_binding",
                        f"Cube '{instance_id}' output '{binding_name}' has no lowered source.",
                        instance_id=instance_id,
                    )
                if binding.node_id not in result:
                    raise CubeInstrumentationError(
                        "execution.instrumentation.missing_output_node",
                        f"Cube '{instance_id}' output source '{binding.node_id}' is absent.",
                        instance_id=instance_id,
                    )
                execution_id = f"{_EXECUTION_PREFIX}:{instance.node_id}:{output_slot}"
                if execution_id in result:
                    raise CubeInstrumentationError(
                        "execution.instrumentation.identity_collision",
                        f"Cube output execution id '{execution_id}' collides with a prompt node.",
                        instance_id=instance_id,
                    )
                result[execution_id] = {
                    "class_type": "SugarCubes.CubeOutput",
                    "inputs": {
                        "value": [binding.node_id, binding.output_index],
                        "cube_id": instance.cube_id,
                        "default_alias": embedded.default_alias,
                        "instance_alias": instance.instance_alias,
                        "instance_id": instance.instance_id,
                    },
                    "_meta": {"title": binding_name},
                }
                owners[execution_id] = ExecutionNodeOwner(instance_id)
                identities.append(
                    CubeOutputIdentity(
                        execution_id=execution_id,
                        instance_id=instance_id,
                        binding=binding_name,
                        root_node_id=instance.node_id,
                        output_slot=output_slot,
                    )
                )
        return InstrumentedPrompt(result, owners, tuple(identities))


def _document_output_names(document: Mapping[str, object] | None) -> tuple[str, ...]:
    """Read author-declared output order from the portable Cube document."""

    if document is None:
        return ()
    implementation = document.get("implementation")
    if not isinstance(implementation, Mapping):
        return ()
    outputs = implementation.get("outputs")
    if not isinstance(outputs, Mapping):
        return ()
    return tuple(str(name) for name in outputs)


def _native_output_names(instance: Mapping[str, object]) -> tuple[str, ...]:
    """Recover saved host output order when no portable document is present."""

    outputs = instance.get("outputs")
    if not isinstance(outputs, Sequence) or isinstance(
        outputs, (str, bytes, bytearray)
    ):
        return ()
    names: list[str] = []
    for entry in outputs:
        if not isinstance(entry, Mapping):
            return ()
        name = entry.get("name")
        if not isinstance(name, str) or not name.strip():
            return ()
        names.append(name.strip())
    return tuple(names)
