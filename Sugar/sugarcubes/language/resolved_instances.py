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
"""Own resolved SugarScript Cube instances and script-facing member lookup."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from typing import Any, Mapping

from ..cube_model import CubeDocument
from .source import SourceSpan
from .syntax import SugarPath, UseStatement


@dataclass
class ResolvedCubeInstance:
    """Hold one mutable semantic instance until compilation freezes the plan."""

    instance_id: str
    alias: str
    document: CubeDocument
    payload: dict[str, Any]
    authored_flavor_id: str
    bypassed: bool
    source_span: SourceSpan


def resolve_instance(
    instances: Mapping[str, ResolvedCubeInstance], alias: str
) -> ResolvedCubeInstance:
    """Resolve one alias case-insensitively while preserving display spelling."""

    try:
        return instances[alias.casefold()]
    except KeyError as error:
        available = ", ".join(instance.alias for instance in instances.values())
        raise KeyError(
            f"Unknown Cube alias '{alias}'. Available aliases: {available}"
        ) from error


def instance_aliases(
    statement: UseStatement,
    alias_base: str,
    repeat_counters: dict[str, int],
) -> tuple[str, ...]:
    """Continue repeat numbering across declarations with the same alias base."""

    if statement.repeat is None:
        return (alias_base,)
    folded = alias_base.casefold()
    start = repeat_counters.get(folded, 0) + 1
    repeat_counters[folded] = start + statement.repeat - 1
    return tuple(
        f"{alias_base}{index}" for index in range(start, start + statement.repeat)
    )


def resolve_field(
    instances: Mapping[str, ResolvedCubeInstance], path: tuple[str, ...]
) -> tuple[ResolvedCubeInstance, str, str]:
    """Resolve a script-facing node/input label to canonical document keys."""

    if len(path) < 3:
        raise ValueError("Set and value references require Cube, node, and input.")
    instance = resolve_instance(instances, path[0])
    node_key = resolve_node(instance, ".".join(path[1:-1]))
    node = implementation_nodes(instance)[node_key]
    input_key = resolve_input(instance.document, node_key, path[-1], node)
    return instance, node_key, input_key


def read_instance_value(
    instances: Mapping[str, ResolvedCubeInstance], path: tuple[str, ...]
) -> object:
    """Read a three-part dotted field reference from current compiled values."""

    instance, node_key, input_key = resolve_field(instances, path)
    inputs = implementation_nodes(instance)[node_key].get("inputs")
    if not isinstance(inputs, Mapping) or input_key not in inputs:
        raise KeyError(f"Field '{'.'.join(path)}' has no current value.")
    return deepcopy(inputs[input_key])


def resolve_node(instance: ResolvedCubeInstance, label: str) -> str:
    """Resolve one node key or user-facing label without suffix guessing."""

    matches = [
        key
        for key, node in implementation_nodes(instance).items()
        if key.casefold() == label.casefold()
        or str(node.get("label", "")).casefold() == label.casefold()
    ]
    if len(matches) != 1:
        raise KeyError(
            f"Node '{label}' is not uniquely defined in Cube '{instance.alias}'."
        )
    return matches[0]


def resolve_input(
    document: CubeDocument,
    node_key: str,
    label: str,
    node: Mapping[str, object],
) -> str:
    """Resolve one surface label, machine key, or materialized input name."""

    candidates = {
        control.input_name
        for control in document.surface.controls
        if control.symbol == node_key
        and (
            control.label.casefold() == label.casefold()
            or control.input_name.casefold() == label.casefold()
        )
    }
    inputs = node.get("inputs")
    if isinstance(inputs, Mapping):
        candidates.update(
            str(key) for key in inputs if str(key).casefold() == label.casefold()
        )
    candidates.update(_declared_input_candidates(document, node, label))
    if len(candidates) != 1:
        raise KeyError(f"Input '{label}' is not uniquely defined on node '{node_key}'.")
    return next(iter(candidates))


def _declared_input_candidates(
    document: CubeDocument,
    node: Mapping[str, object],
    label: str,
) -> set[str]:
    """Resolve inputs declared by embedded native or ordinary node definitions."""

    class_type = node.get("class_type")
    if not isinstance(class_type, str):
        return set()
    candidates: set[str] = set()
    for subgraph in document.implementation.subgraphs:
        if subgraph.get("id") != class_type:
            continue
        raw_inputs = subgraph.get("inputs")
        if not isinstance(raw_inputs, list):
            continue
        for entry in raw_inputs:
            if not isinstance(entry, Mapping):
                continue
            name = entry.get("name")
            if isinstance(name, str) and _matches_input_label(entry, name, label):
                candidates.add(name)

    definition = document.implementation.definitions.get(class_type)
    raw_groups = definition.get("input") if isinstance(definition, Mapping) else None
    if isinstance(raw_groups, Mapping):
        for group in raw_groups.values():
            if not isinstance(group, Mapping):
                continue
            candidates.update(
                str(name) for name in group if str(name).casefold() == label.casefold()
            )
    return candidates


def _matches_input_label(
    entry: Mapping[str, object], name: str, requested: str
) -> bool:
    """Match one canonical native input name or persisted presentation label."""

    aliases = (name, entry.get("label"), entry.get("localized_name"))
    return any(
        isinstance(alias, str) and alias.casefold() == requested.casefold()
        for alias in aliases
    )


def resolve_boundary(
    instance: ResolvedCubeInstance,
    parts: tuple[str, ...],
    direction: str,
) -> str:
    """Resolve a public boundary by canonical name or unique final label."""

    if not parts:
        raise ValueError(f"Connection {direction[:-1]} must name a boundary.")
    implementation = instance.payload.get("implementation")
    boundaries = (
        implementation.get(direction) if isinstance(implementation, Mapping) else None
    )
    if not isinstance(boundaries, Mapping):
        raise ValueError(f"Cube '{instance.alias}' has invalid {direction}.")
    requested = ".".join(parts)
    matches = [
        str(key)
        for key in boundaries
        if str(key).casefold() == requested.casefold()
        or str(key).split(".")[-1].casefold() == parts[-1].casefold()
    ]
    if len(matches) != 1:
        raise KeyError(
            f"Boundary '{requested}' is not uniquely defined in Cube '{instance.alias}'."
        )
    return matches[0]


def implementation_nodes(instance: ResolvedCubeInstance) -> dict[str, dict[str, Any]]:
    """Return the mutable validated implementation node map for one instance."""

    implementation = instance.payload.get("implementation")
    nodes = implementation.get("nodes") if isinstance(implementation, dict) else None
    if not isinstance(nodes, dict):
        raise ValueError(f"Cube '{instance.alias}' has invalid implementation nodes.")
    return nodes


def expand_path(path: SugarPath) -> tuple[SugarPath, ...]:
    """Expand an optional alias range into exact immutable paths."""

    if path.alias_range is None:
        return (path,)
    return tuple(
        SugarPath((f"{path.parts[0]}{index}", *path.parts[1:]), None, path.span)
        for index in range(path.alias_range.start, path.alias_range.end + 1)
    )
