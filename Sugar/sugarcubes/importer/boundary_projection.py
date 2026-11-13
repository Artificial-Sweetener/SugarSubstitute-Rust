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
"""Project legacy Cube marker bindings into durable native-boundary data."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any, TypedDict


class CubeBoundaryEndpoint(TypedDict):
    """Describe one internal node endpoint owned by a Cube boundary."""

    symbol: str
    input: str


class CubeOutputBoundaryEndpoint(TypedDict):
    """Describe one internal output endpoint owned by a Cube boundary."""

    symbol: str
    slot: int


class CubeInputBoundary(TypedDict):
    """Describe one canonical Cube input boundary."""

    id: str
    name: str
    label: str
    type: str
    targets: list[CubeBoundaryEndpoint]


class CubeOutputBoundary(TypedDict):
    """Describe one canonical Cube output boundary."""

    id: str
    name: str
    label: str
    type: str
    source: CubeOutputBoundaryEndpoint


class CubeBoundaryProjection(TypedDict):
    """Carry a Cube's complete native boundary contract."""

    inputs: list[CubeInputBoundary]
    outputs: list[CubeOutputBoundary]


def empty_cube_boundary_projection() -> CubeBoundaryProjection:
    """Create the empty canonical boundary contract without widening its type."""

    return {"inputs": [], "outputs": []}


def project_cube_boundaries(
    *,
    nodes: Mapping[str, Any],
    inputs: Mapping[str, Any],
    outputs: Mapping[str, Any],
    definitions: Mapping[str, Any],
    subgraphs: Sequence[Mapping[str, Any]],
    marker_titles: Mapping[str, str],
    warnings: list[str],
) -> CubeBoundaryProjection:
    """Derive typed native boundaries from a legacy Cube without mutating it."""

    type_lookup = _BoundaryTypeLookup(nodes, definitions, subgraphs)
    input_boundaries: list[CubeInputBoundary] = []
    output_boundaries: list[CubeOutputBoundary] = []

    for alias, spec in inputs.items():
        targets = _read_input_targets(spec)
        concrete_types = {
            port_type
            for target in targets
            if (port_type := type_lookup.input_type(target["symbol"], target["input"]))
        }
        boundary_type = _resolve_boundary_type(concrete_types, alias, "input", warnings)
        input_boundaries.append(
            {
                "id": alias,
                "name": alias,
                "label": marker_titles.get(alias) or alias,
                "type": boundary_type,
                "targets": targets,
            }
        )

    for alias, spec in outputs.items():
        source = _read_output_source(spec)
        boundary_type = type_lookup.output_type(source["symbol"], source["slot"])
        if not boundary_type:
            warnings.append(
                f"Cube output '{alias}' has no declared type; using wildcard boundary."
            )
            boundary_type = "*"
        output_boundaries.append(
            {
                "id": alias,
                "name": alias,
                "label": marker_titles.get(alias) or alias,
                "type": boundary_type,
                "source": source,
            }
        )

    return {"inputs": input_boundaries, "outputs": output_boundaries}


class _BoundaryTypeLookup:
    """Resolve declarations from persisted node schemas and nested subgraphs."""

    def __init__(
        self,
        nodes: Mapping[str, Any],
        definitions: Mapping[str, Any],
        subgraphs: Sequence[Mapping[str, Any]],
    ) -> None:
        self._nodes = nodes
        self._definitions = definitions
        self._subgraphs = {
            subgraph_id: entry
            for entry in subgraphs
            if (subgraph_id := _read_string(entry.get("id")))
        }

    def input_type(self, symbol: str, input_name: str) -> str:
        """Return one declared input type for an internal endpoint."""

        node = self._nodes.get(symbol)
        class_type = _read_node_class_type(node)
        if not class_type:
            return ""
        subgraph = self._subgraphs.get(class_type)
        if subgraph is not None:
            return _read_named_io_type(subgraph.get("inputs"), input_name)
        return _read_definition_input_type(
            self._definitions.get(class_type), input_name
        )

    def output_type(self, symbol: str, slot: int) -> str:
        """Return one declared output type for an internal endpoint."""

        node = self._nodes.get(symbol)
        class_type = _read_node_class_type(node)
        if not class_type:
            return ""
        subgraph = self._subgraphs.get(class_type)
        if subgraph is not None:
            return _read_indexed_io_type(subgraph.get("outputs"), slot)
        return _read_definition_output_type(self._definitions.get(class_type), slot)


def _read_input_targets(spec: Any) -> list[CubeBoundaryEndpoint]:
    """Normalize one legacy input spec into explicit target endpoints."""

    raw_targets = getattr(spec, "targets", ())
    result: list[CubeBoundaryEndpoint] = []
    if not isinstance(raw_targets, Sequence):
        return result
    for target in raw_targets:
        if not isinstance(target, tuple) or len(target) != 2:
            continue
        symbol = _read_string(target[0])
        input_name = _read_string(target[1])
        if symbol and input_name:
            result.append({"symbol": symbol, "input": input_name})
    return result


def _read_output_source(spec: Any) -> CubeOutputBoundaryEndpoint:
    """Normalize one legacy output spec into its source endpoint."""

    symbol = _read_string(getattr(spec, "source_symbol", ""))
    raw_slot = getattr(spec, "source_slot", 0)
    slot = raw_slot if isinstance(raw_slot, int) and raw_slot >= 0 else 0
    return {"symbol": symbol, "slot": slot}


def _resolve_boundary_type(
    concrete_types: set[str],
    alias: str,
    direction: str,
    warnings: list[str],
) -> str:
    """Return one compatible type without widening a single known declaration."""

    if len(concrete_types) == 1:
        return next(iter(concrete_types))
    if len(concrete_types) > 1:
        warnings.append(
            f"Cube {direction} '{alias}' has incompatible declared types; using wildcard boundary."
        )
    else:
        warnings.append(
            f"Cube {direction} '{alias}' has no declared type; using wildcard boundary."
        )
    return "*"


def _read_named_io_type(entries: Any, name: str) -> str:
    """Read a named nested-subgraph boundary type."""

    if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
        return ""
    for entry in entries:
        if isinstance(entry, Mapping) and _read_string(entry.get("name")) == name:
            return _read_type(entry.get("type"))
    return ""


def _read_indexed_io_type(entries: Any, slot: int) -> str:
    """Read an indexed nested-subgraph boundary type."""

    if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
        return ""
    if slot < 0 or slot >= len(entries):
        return ""
    entry = entries[slot]
    return _read_type(entry.get("type")) if isinstance(entry, Mapping) else ""


def _read_definition_input_type(definition: Any, input_name: str) -> str:
    """Read one Comfy node input type from its saved object-info definition."""

    if not isinstance(definition, Mapping):
        return ""
    input_groups = definition.get("input")
    if not isinstance(input_groups, Mapping):
        return ""
    for group in input_groups.values():
        if not isinstance(group, Mapping):
            continue
        entry = group.get(input_name)
        if (
            isinstance(entry, Sequence)
            and not isinstance(entry, (str, bytes))
            and entry
        ):
            return _read_type(entry[0])
    return ""


def _read_definition_output_type(definition: Any, slot: int) -> str:
    """Read one Comfy node output type from its saved object-info definition."""

    if not isinstance(definition, Mapping):
        return ""
    outputs = definition.get("output")
    if not isinstance(outputs, Sequence) or isinstance(outputs, (str, bytes)):
        return ""
    if slot < 0 or slot >= len(outputs):
        return ""
    return _read_type(outputs[slot])


def _read_type(value: Any) -> str:
    """Return a concrete serialized port type or an empty value."""

    value_string = _read_string(value)
    return value_string if value_string and value_string != "*" else ""


def _read_node_class_type(node: Any) -> str:
    """Read an internal class type from a parsed node or serialized record."""

    if isinstance(node, Mapping):
        return _read_string(node.get("class_type"))
    return _read_string(getattr(node, "class_type", ""))


def _read_string(value: Any) -> str:
    """Normalize one dynamic persisted string."""

    return value.strip() if isinstance(value, str) else ""
