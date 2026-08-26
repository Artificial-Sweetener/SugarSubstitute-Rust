#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Lower one validated Cube document and resolve its execution modes."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from typing import Mapping

from ..cube_model import CubeDocument
from .errors import CubeLoweringError
from .lowering_values import (
    connection,
    definition_input_types,
    node_mode,
    output_endpoint,
    string_list,
)
from .models import BoundaryBinding
from .native_cube_definition_lowering import NativeCubeDefinitionLowerer
from .native_subgraph_lowering import NativeSubgraphDocumentLowerer
from .portable_boundary_lowering import (
    describe_document_bindings,
    execution_node_id,
)

_DISABLED_MODE = 2
_BYPASS_MODE = 4


@dataclass(frozen=True)
class CubeDocumentLowering:
    """Carry one Cube's prompt projection and resolved public endpoints."""

    prompt: dict[str, dict[str, object]]
    node_definitions: dict[str, Mapping[str, object]]
    boundary_bindings: tuple[BoundaryBinding, ...]


class CubeDocumentLowerer:
    """Own deterministic internal expansion and bypass rewiring for one Cube."""

    def lower(
        self,
        instance_id: str,
        document: CubeDocument | None,
        native_definition: Mapping[str, object] | None = None,
        native_instance: Mapping[str, object] | None = None,
        connected_input_bindings: frozenset[str] = frozenset(),
        native_subgraphs: tuple[Mapping[str, object], ...] = (),
        native_node_definitions: Mapping[str, object] | None = None,
    ) -> CubeDocumentLowering:
        """Expand one Cube implementation into stable execution node identities."""

        native = NativeCubeDefinitionLowerer()
        if (
            native_definition is not None
            and native_instance is not None
            and native.supports(native_definition)
        ):
            lowered = native.lower(
                instance_id,
                document,
                native_definition,
                native_instance,
                connected_input_bindings,
                native_subgraphs,
                native_node_definitions,
            )
            return CubeDocumentLowering(
                prompt=lowered.prompt,
                node_definitions=lowered.node_definitions,
                boundary_bindings=lowered.boundary_bindings,
            )

        if document is None:
            raise CubeLoweringError(
                "execution.lowering.portable_definition_missing",
                f"Cube instance '{instance_id}' has neither executable native state nor a validated embedded Cube document.",
            )

        if document.implementation.subgraphs:
            endpoints = tuple(
                endpoint
                for value in document.implementation.outputs.values()
                if (endpoint := output_endpoint(value)) is not None
            )
            native_prompt, native_definitions, resolved_outputs = (
                NativeSubgraphDocumentLowerer().lower(instance_id, document, endpoints)
            )
            return CubeDocumentLowering(
                prompt=native_prompt,
                node_definitions=native_definitions,
                boundary_bindings=describe_document_bindings(
                    instance_id,
                    document,
                    prompt=native_prompt,
                    resolved_outputs=resolved_outputs,
                ),
            )

        nodes = document.implementation.nodes
        class_definitions = document.implementation.definitions
        skipped = {
            symbol: node_mode(node)
            for symbol, node in nodes.items()
            if node_mode(node) in {_DISABLED_MODE, _BYPASS_MODE}
        }
        bypass_sources = _bypass_sources(nodes, class_definitions, skipped)
        prompt: dict[str, dict[str, object]] = {}
        definitions: dict[str, Mapping[str, object]] = {}
        for symbol in sorted(nodes):
            raw_node = nodes[symbol]
            if symbol in skipped:
                continue
            class_type = raw_node.get("class_type")
            if not isinstance(class_type, str) or not class_type.strip():
                raise CubeLoweringError(
                    "execution.lowering.invalid_class_type",
                    f"Cube node '{symbol}' has no valid class type.",
                    path=f"$.implementation.nodes.{symbol}.class_type",
                )
            raw_inputs = raw_node.get("inputs", {})
            if not isinstance(raw_inputs, Mapping):
                raise CubeLoweringError(
                    "execution.lowering.invalid_inputs",
                    f"Cube node '{symbol}' inputs must be an object.",
                    path=f"$.implementation.nodes.{symbol}.inputs",
                )
            lowered_inputs = {
                str(name): _lower_input_value(
                    value,
                    instance_id=instance_id,
                    nodes=nodes,
                    skipped=skipped,
                    bypass_sources=bypass_sources,
                )
                for name, value in raw_inputs.items()
            }
            node_id = execution_node_id(instance_id, symbol)
            prompt[node_id] = {
                "class_type": class_type.strip(),
                "inputs": lowered_inputs,
            }
            definition = class_definitions.get(class_type)
            definitions[node_id] = (
                deepcopy(dict(definition)) if isinstance(definition, Mapping) else {}
            )
        return CubeDocumentLowering(
            prompt=prompt,
            node_definitions=definitions,
            boundary_bindings=describe_document_bindings(
                instance_id, document, prompt=prompt
            ),
        )

    def describe_bindings(
        self,
        instance_id: str,
        document: CubeDocument | None,
        native_definition: Mapping[str, object] | None = None,
        native_instance: Mapping[str, object] | None = None,
    ) -> tuple[BoundaryBinding, ...]:
        """Describe active native or portable boundaries for a skipped Cube."""

        native = NativeCubeDefinitionLowerer()
        if (
            native_definition is not None
            and native_instance is not None
            and native.supports(native_definition)
        ):
            return native.describe(instance_id, document, native_instance)
        if document is None:
            raise CubeLoweringError(
                "execution.lowering.portable_definition_missing",
                f"Cube instance '{instance_id}' has neither executable native state nor a validated embedded Cube document.",
            )
        return describe_document_bindings(instance_id, document)


def _bypass_sources(
    nodes: Mapping[str, Mapping[str, object]],
    definitions: Mapping[str, object],
    skipped: Mapping[str, int],
) -> dict[tuple[str, int], object]:
    """Map bypassed outputs to one type-compatible connected input."""

    result: dict[tuple[str, int], object] = {}
    for symbol, mode in skipped.items():
        if mode != _BYPASS_MODE:
            continue
        node = nodes[symbol]
        class_type = node.get("class_type")
        definition = (
            definitions.get(class_type) if isinstance(class_type, str) else None
        )
        if not isinstance(definition, Mapping):
            continue
        output_types = string_list(definition.get("output"))
        inputs = node.get("inputs")
        if not isinstance(inputs, Mapping):
            continue
        input_types = definition_input_types(definition)
        for output_index, output_type in enumerate(output_types):
            candidates = [
                value
                for name, value in inputs.items()
                if input_types.get(str(name)) == output_type
                and connection(value) is not None
            ]
            if len(candidates) == 1:
                result[(symbol, output_index)] = candidates[0]
    return result


def _lower_input_value(
    value: object,
    *,
    instance_id: str,
    nodes: Mapping[str, Mapping[str, object]],
    skipped: Mapping[str, int],
    bypass_sources: Mapping[tuple[str, int], object],
) -> object:
    """Remap one internal connection and resolve every bypass chain."""

    parsed_connection = connection(value)
    if parsed_connection is None:
        return deepcopy(value)
    symbol, output_index = parsed_connection
    seen: set[tuple[str, int]] = set()
    while symbol in skipped:
        key = (symbol, output_index)
        if key in seen:
            raise CubeLoweringError(
                "execution.lowering.bypass_cycle",
                f"Bypass chain cycles at node '{symbol}'.",
            )
        seen.add(key)
        replacement = bypass_sources.get(key)
        if replacement is None:
            return None
        next_connection = connection(replacement)
        if next_connection is None:
            return deepcopy(replacement)
        symbol, output_index = next_connection
    if symbol not in nodes:
        raise CubeLoweringError(
            "execution.lowering.unknown_internal_source",
            f"Internal connection references unknown Cube node '{symbol}'.",
        )
    return [execution_node_id(instance_id, symbol), output_index]
