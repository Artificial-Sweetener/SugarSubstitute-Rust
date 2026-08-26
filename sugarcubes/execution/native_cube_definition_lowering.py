#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project a saved native Cube definition into executable prompt ownership."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from ..cube_model import CubeDocument
from .errors import CubeLoweringError
from .lowering_values import connection
from .models import BoundaryBinding
from .native_subgraph_lowering import NativeSubgraphDocumentLowerer

Prompt = dict[str, dict[str, object]]
DefinitionIndex = dict[str, Mapping[str, object]]
_MARKER_OWNER = "@binding"


@dataclass(frozen=True)
class NativeCubeDefinitionLowering:
    """Carry one native definition's prompt and public boundary projection."""

    prompt: Prompt
    node_definitions: DefinitionIndex
    boundary_bindings: tuple[BoundaryBinding, ...]


class NativeCubeDefinitionLowerer:
    """Own saved-definition precedence and native/public boundary aliases."""

    def supports(self, definition: Mapping[str, object]) -> bool:
        """Return whether a definition contains an executable native graph."""

        return bool(_entries(definition, "nodes"))

    def lower(
        self,
        instance_id: str,
        document: CubeDocument | None,
        definition: Mapping[str, object],
        instance: Mapping[str, object],
        connected_input_bindings: frozenset[str],
        native_subgraphs: Sequence[Mapping[str, object]] = (),
        native_node_definitions: Mapping[str, object] | None = None,
    ) -> NativeCubeDefinitionLowering:
        """Lower current native state while retaining portable semantic aliases."""

        instance_inputs = _entries(instance, "inputs")
        external_inputs = {
            entry.name: [_MARKER_OWNER, entry.name]
            for entry in instance_inputs
            if entry.name in connected_input_bindings
        }
        prompt, node_definitions, outputs = (
            NativeSubgraphDocumentLowerer().lower_definition(
                instance_id,
                document,
                definition,
                external_inputs,
                native_subgraphs,
                native_node_definitions,
            )
        )
        return NativeCubeDefinitionLowering(
            prompt=prompt,
            node_definitions=node_definitions,
            boundary_bindings=_bindings(
                instance_id,
                document,
                instance,
                prompt=prompt,
                resolved_outputs=outputs,
            ),
        )

    def describe(
        self,
        instance_id: str,
        document: CubeDocument | None,
        instance: Mapping[str, object],
    ) -> tuple[BoundaryBinding, ...]:
        """Describe skipped native boundaries without inventing live endpoints."""

        return _bindings(instance_id, document, instance)


@dataclass(frozen=True)
class _BoundaryEntry:
    """Retain one validated host boundary name, type, and slot."""

    name: str
    value_type: str
    slot: int


def _bindings(
    instance_id: str,
    document: CubeDocument | None,
    instance: Mapping[str, object],
    *,
    prompt: Mapping[str, Mapping[str, object]] | None = None,
    resolved_outputs: tuple[object, ...] = (),
) -> tuple[BoundaryBinding, ...]:
    """Map host names and stable document aliases to the same native endpoints."""

    result: list[BoundaryBinding] = []
    active_prompt = prompt or {}
    document_inputs = (
        tuple(document.implementation.inputs) if document is not None else ()
    )
    for entry in _entries(instance, "inputs"):
        targets = _marker_targets(active_prompt, entry.name)
        if not targets:
            targets = [(f"{instance_id}:@input:{entry.slot}", entry.name)]
        names = _aliases(entry.name, document_inputs, entry.slot)
        for binding_name in names:
            result.extend(
                BoundaryBinding(
                    instance_id=instance_id,
                    binding=binding_name,
                    direction="input",
                    node_id=node_id,
                    input_name=input_name,
                    value_type=entry.value_type,
                )
                for node_id, input_name in targets
            )

    document_outputs = (
        tuple(document.implementation.outputs) if document is not None else ()
    )
    for entry in _entries(instance, "outputs"):
        resolved = (
            resolved_outputs[entry.slot] if entry.slot < len(resolved_outputs) else None
        )
        endpoint = connection(resolved)
        node_id, output_index = (
            endpoint
            if endpoint is not None
            else (f"{instance_id}:@output:{entry.slot}", entry.slot)
        )
        for binding_name in _aliases(entry.name, document_outputs, entry.slot):
            result.append(
                BoundaryBinding(
                    instance_id=instance_id,
                    binding=binding_name,
                    direction="output",
                    node_id=node_id,
                    output_index=output_index,
                    value_type=entry.value_type,
                )
            )
    return tuple(result)


def _entries(payload: Mapping[str, object], key: str) -> tuple[_BoundaryEntry, ...]:
    """Validate ordered native boundary arrays without accepting ambiguous names."""

    value = payload.get(key)
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise CubeLoweringError(
            "execution.lowering.invalid_native_definition",
            f"Native Cube definition '{key}' must be an array.",
        )
    result: list[_BoundaryEntry] = []
    names: set[str] = set()
    for slot, raw_entry in enumerate(value):
        if not isinstance(raw_entry, Mapping):
            if key == "nodes":
                continue
            raise _invalid_boundary(key, slot)
        if key == "nodes":
            result.append(_BoundaryEntry(str(slot), "", slot))
            continue
        name = raw_entry.get("name")
        if not isinstance(name, str) or not name.strip() or name.strip() in names:
            raise _invalid_boundary(key, slot)
        normalized_name = name.strip()
        names.add(normalized_name)
        value_type = raw_entry.get("type")
        result.append(
            _BoundaryEntry(
                normalized_name,
                value_type if isinstance(value_type, str) else "",
                slot,
            )
        )
    return tuple(result)


def _marker_targets(
    prompt: Mapping[str, Mapping[str, object]], binding: str
) -> list[tuple[str, str]]:
    """Locate every flattened consumer of one native public input."""

    marker = [_MARKER_OWNER, binding]
    return sorted(
        (node_id, str(name))
        for node_id, node in prompt.items()
        for inputs in (node.get("inputs"),)
        if isinstance(inputs, Mapping)
        for name, value in inputs.items()
        if value == marker
    )


def _aliases(
    native_name: str, document_names: tuple[str, ...], slot: int
) -> tuple[str, ...]:
    """Expose the native host name and portable semantic name once each."""

    names = [native_name]
    if slot < len(document_names) and document_names[slot] not in names:
        names.append(document_names[slot])
    return tuple(names)


def _invalid_boundary(direction: str, slot: int) -> CubeLoweringError:
    """Create one stable malformed or duplicate boundary diagnostic."""

    return CubeLoweringError(
        "execution.lowering.invalid_native_boundary",
        f"Native Cube {direction} boundary {slot} has no unique stable name.",
    )
