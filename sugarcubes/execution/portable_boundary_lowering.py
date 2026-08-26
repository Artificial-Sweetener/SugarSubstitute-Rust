#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project portable Cube boundaries onto lowered execution endpoints."""

from __future__ import annotations

from collections.abc import Mapping

from ..cube_model import CubeDocument
from .errors import CubeLoweringError
from .lowering_values import (
    connection,
    definition_input_types,
    input_targets,
    output_endpoint,
    string_list,
)
from .models import BoundaryBinding


def describe_document_bindings(
    instance_id: str,
    document: CubeDocument,
    *,
    prompt: Mapping[str, Mapping[str, object]] | None = None,
    resolved_outputs: Mapping[tuple[str, int], object] | None = None,
) -> tuple[BoundaryBinding, ...]:
    """Describe canonical boundaries and concrete active endpoints when available."""

    prompt = prompt or {}
    outputs = resolved_outputs or {}
    result: list[BoundaryBinding] = []
    for binding, value in document.implementation.inputs.items():
        targets = [
            (execution_node_id(instance_id, symbol), input_name)
            for symbol, input_name in input_targets(value)
        ]
        concrete = [target for target in targets if target[0] in prompt]
        concrete.extend(_marker_targets(prompt, binding))
        selected = sorted(set(concrete)) or targets
        value_type = _input_binding_type(document, value)
        result.extend(
            BoundaryBinding(
                instance_id=instance_id,
                binding=binding,
                direction="input",
                node_id=node_id,
                input_name=input_name,
                value_type=value_type,
            )
            for node_id, input_name in selected
        )
    for binding, value in document.implementation.outputs.items():
        endpoint = output_endpoint(value)
        if endpoint is None:
            raise CubeLoweringError(
                "execution.lowering.invalid_output_binding",
                f"Cube output binding '{binding}' has no valid source endpoint.",
                path=f"$.implementation.outputs.{binding}",
            )
        resolved_connection = connection(outputs.get(endpoint))
        node_id, output_index = (
            resolved_connection
            if resolved_connection is not None
            else (execution_node_id(instance_id, endpoint[0]), endpoint[1])
        )
        result.append(
            BoundaryBinding(
                instance_id=instance_id,
                binding=binding,
                direction="output",
                node_id=node_id,
                output_index=output_index,
                value_type=_output_binding_type(document, endpoint),
            )
        )
    return tuple(result)


def execution_node_id(instance_id: str, symbol: str) -> str:
    """Build a stable readable execution identity from canonical owners."""

    return f"{instance_id}:{symbol}"


def _marker_targets(
    prompt: Mapping[str, Mapping[str, object]], binding: str
) -> list[tuple[str, str]]:
    """Find flattened native inputs that retain one canonical binding marker."""

    marker = ["@binding", binding]
    return [
        (node_id, str(name))
        for node_id, node in prompt.items()
        for inputs in (node.get("inputs"),)
        if isinstance(inputs, Mapping)
        for name, value in inputs.items()
        if value == marker
    ]


def _input_binding_type(document: CubeDocument, value: object) -> str:
    """Resolve one public input type from its first internally typed target."""

    candidates = {
        _root_input_type(document, symbol, input_name)
        for symbol, input_name in input_targets(value)
    }
    normalized = sorted(value_type for value_type in candidates if value_type)
    return normalized[0] if len(set(normalized)) == 1 else ""


def _output_binding_type(document: CubeDocument, endpoint: tuple[str, int]) -> str:
    """Resolve one public output type from its root implementation endpoint."""

    node = document.implementation.nodes.get(endpoint[0])
    class_type = node.get("class_type") if node is not None else None
    if not isinstance(class_type, str):
        return ""
    output_types = _class_output_types(document, class_type)
    return output_types[endpoint[1]] if endpoint[1] < len(output_types) else ""


def _root_input_type(document: CubeDocument, symbol: str, input_name: str) -> str:
    """Resolve one root input type from ordinary or native definitions."""

    node = document.implementation.nodes.get(symbol)
    class_type = node.get("class_type") if node is not None else None
    if not isinstance(class_type, str):
        return ""
    definition = document.implementation.definitions.get(class_type)
    if isinstance(definition, Mapping):
        return definition_input_types(definition).get(input_name, "")
    subgraph = _subgraph(document, class_type)
    entries = subgraph.get("inputs") if subgraph is not None else None
    if not isinstance(entries, list):
        return ""
    for entry in entries:
        if isinstance(entry, Mapping) and entry.get("name") == input_name:
            value_type = entry.get("type")
            return value_type if isinstance(value_type, str) else ""
    return ""


def _class_output_types(document: CubeDocument, class_type: str) -> tuple[str, ...]:
    """Read ordinary or native-subgraph output types without host imports."""

    definition = document.implementation.definitions.get(class_type)
    if isinstance(definition, Mapping):
        return string_list(definition.get("output"))
    subgraph = _subgraph(document, class_type)
    entries = subgraph.get("outputs") if subgraph is not None else None
    if not isinstance(entries, list):
        return ()
    return tuple(
        value_type
        for entry in entries
        if isinstance(entry, Mapping)
        and isinstance((value_type := entry.get("type")), str)
    )


def _subgraph(document: CubeDocument, class_type: str) -> Mapping[str, object] | None:
    """Find one portable nested definition by stable class identity."""

    return next(
        (
            value
            for value in document.implementation.subgraphs
            if value.get("id") == class_type
        ),
        None,
    )
