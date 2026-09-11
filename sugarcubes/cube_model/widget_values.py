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
"""Own stable widget identities and their persisted cube representation."""

from __future__ import annotations

from collections.abc import Mapping, Sequence, Set as AbstractSet
from copy import deepcopy
from dataclasses import dataclass
from typing import Any, TypeGuard

from .input_persistence import should_store_authored_value
from .picker_fields import (
    find_input_field_spec,
    is_unselected_picker_value,
    widget_input_names,
)
from .subgraph_boundary_widgets import index_boundary_widget_names

WORKFLOW_WIDGET_VALUES_KEY = "sugarcubes_widget_values"
_CONTROL_AFTER_GENERATE_VALUES = frozenset(
    {"fixed", "increment", "decrement", "randomize"}
)


class WidgetSnapshotError(ValueError):
    """Reject positional widget data whose input relationships are ambiguous."""


@dataclass(frozen=True)
class WidgetSnapshot:
    """Hold name-addressed widget values and their decoding provenance."""

    values: dict[str, Any]
    source: str


def decode_workflow_widget_snapshot(
    node: Mapping[str, Any],
    definition: Mapping[str, Any],
    *,
    included_linked_names: AbstractSet[str] = frozenset(),
) -> WidgetSnapshot | None:
    """Decode request or same-snapshot workflow widget values by input name."""

    explicit_values = node.get(WORKFLOW_WIDGET_VALUES_KEY)
    if isinstance(explicit_values, Mapping):
        return WidgetSnapshot(
            values=_normalize_explicit_values(explicit_values),
            source="live_name_map",
        )

    widget_values = node.get("widgets_values")
    if not _is_sequence(widget_values):
        return None
    names = serialized_widget_names(node, included_linked_names=included_linked_names)
    linked_names = _linked_widget_names(node) - included_linked_names
    if not widget_values and names and set(names).issubset(included_linked_names):
        return WidgetSnapshot(values={}, source="serialized_inputs")
    if widget_values and included_linked_names:
        try:
            values_with_boundary_defaults = _decode_positional_values(
                names,
                widget_values,
                definition,
            )
        except WidgetSnapshotError:
            local_names = [name for name in names if name not in included_linked_names]
            try:
                local_values = _decode_positional_values(
                    local_names,
                    widget_values,
                    definition,
                )
            except WidgetSnapshotError:
                pass
            else:
                return WidgetSnapshot(
                    values=local_values,
                    source="serialized_inputs_without_boundary_defaults",
                )
        else:
            return WidgetSnapshot(
                values=values_with_boundary_defaults,
                source="serialized_inputs",
            )
    if widget_values and linked_names:
        try:
            named_values = _decode_positional_values(names, widget_values, definition)
        except WidgetSnapshotError:
            same_snapshot_names = serialized_widget_names(
                node,
                included_linked_names=_linked_widget_names(node),
            )
            values = _decode_positional_values(
                same_snapshot_names,
                widget_values,
                definition,
            )
            return WidgetSnapshot(
                values={name: values[name] for name in names if name in values},
                source="serialized_inputs_with_link_anchors",
            )
        else:
            return WidgetSnapshot(values=named_values, source="serialized_inputs")
    if not names and not widget_values:
        return WidgetSnapshot(values={}, source="serialized_inputs")
    return WidgetSnapshot(
        values=_decode_positional_values(names, widget_values, definition),
        source="serialized_inputs",
    )


def decode_versioned_widget_snapshot(
    node: Mapping[str, Any],
    definition: Mapping[str, Any],
    *,
    included_linked_names: AbstractSet[str] = frozenset(),
    versioned_widget_names: Sequence[str] | None = None,
) -> WidgetSnapshot | None:
    """Decode a snapshot using an explicitly co-versioned node definition.

    This fallback is valid only when the caller has resolved the exact Cube
    version that produced ``node``. Linked widget identities from the saved
    node are removed because Comfy omits their values from ``widgets_values``.
    """

    try:
        return decode_workflow_widget_snapshot(
            node,
            definition,
            included_linked_names=included_linked_names,
        )
    except WidgetSnapshotError as original_error:
        values = node.get("widgets_values")
        if not _is_sequence(values) or not values:
            raise
        linked_names = _linked_widget_names(node) - included_linked_names
        exact_names = (
            list(versioned_widget_names)
            if versioned_widget_names is not None
            else widget_input_names(definition)
        )
        versioned_names = [name for name in exact_names if name not in linked_names]
        candidates = [versioned_names]
        if included_linked_names:
            candidates.append(
                [name for name in versioned_names if name not in included_linked_names]
            )
        for names in candidates:
            try:
                decoded = _decode_positional_values(names, values, definition)
            except WidgetSnapshotError:
                continue
            return WidgetSnapshot(decoded, "exact_versioned_definition")
        raise original_error


def decode_versioned_surface_snapshot(
    node: Mapping[str, Any],
    definition: Mapping[str, Any],
    control_names: Sequence[str],
) -> WidgetSnapshot | None:
    """Decode only exact Cube surface fields and ignore non-semantic host widgets.

    Cube surface identities are co-versioned with the saved node. Some custom
    nodes serialize additional presentation-only widgets after their executable
    inputs; those trailing values are deliberately outside the Cube surface
    contract and cannot affect execution.
    """

    normalized_names = _unique_names(control_names)
    explicit_values = node.get(WORKFLOW_WIDGET_VALUES_KEY)
    if isinstance(explicit_values, Mapping):
        values = _normalize_explicit_values(explicit_values)
        return WidgetSnapshot(
            values={name: values[name] for name in normalized_names if name in values},
            source="live_surface_name_map",
        )
    widget_values = node.get("widgets_values")
    if not _is_sequence(widget_values):
        return None
    try:
        snapshot = decode_workflow_widget_snapshot(node, definition)
    except WidgetSnapshotError:
        snapshot = None
    if snapshot is not None:
        return WidgetSnapshot(
            values={
                name: snapshot.values[name]
                for name in normalized_names
                if name in snapshot.values
            },
            source=snapshot.source,
        )
    linked_names = _linked_widget_names(node)
    positional_names = [name for name in normalized_names if name not in linked_names]
    return WidgetSnapshot(
        values=_decode_positional_prefix(positional_names, widget_values, definition),
        source="exact_versioned_surface",
    )


def serialized_widget_names(
    node: Mapping[str, Any],
    *,
    included_linked_names: AbstractSet[str] = frozenset(),
) -> list[str]:
    """Return widget identities stored in the same workflow node snapshot."""

    inputs = node.get("inputs")
    if not _is_sequence(inputs):
        return []
    names: list[str] = []
    for entry in inputs:
        if not isinstance(entry, Mapping):
            continue
        name = _widget_input_name(entry)
        if entry.get("link") is not None and name not in included_linked_names:
            continue
        if name is not None:
            names.append(name)
    return names


def canonicalize_subgraph_widget_values(
    subgraphs: Sequence[Mapping[str, Any]],
    definitions: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Persist subgraph widget values one-for-one by their snapshot names.

    Machine-local and volatile values become ``None`` placeholders. The existing
    ``widgets_values`` array remains schema-compatible while import can rebuild
    the host's current positional array from the adjacent named widget inputs.
    """

    canonical = [deepcopy(dict(subgraph)) for subgraph in subgraphs]
    for subgraph in canonical:
        boundary_names_by_node = index_boundary_widget_names(subgraph)
        nodes = subgraph.get("nodes")
        if not _is_sequence(nodes):
            continue
        for node in nodes:
            if not isinstance(node, dict):
                continue
            class_type = node.get("type", node.get("class_type"))
            if not isinstance(class_type, str):
                continue
            definition = definitions.get(class_type)
            live_definition = definition if isinstance(definition, Mapping) else {}
            node_id = node.get("id")
            boundary_names = (
                boundary_names_by_node.get(node_id, frozenset())
                if isinstance(node_id, str | int) and not isinstance(node_id, bool)
                else frozenset()
            )
            try:
                snapshot = decode_workflow_widget_snapshot(
                    node,
                    live_definition,
                    included_linked_names=boundary_names,
                )
            except WidgetSnapshotError as exc:
                raise WidgetSnapshotError(
                    f"node_id={node.get('id')!r}; class_type={class_type}; {exc}"
                ) from exc
            if snapshot is None:
                continue
            if snapshot.source in {
                "live_name_map",
            }:
                _attach_explicit_widget_identities(
                    node,
                    snapshot.values,
                    live_definition,
                )
            names = serialized_widget_names(
                node,
                included_linked_names=boundary_names,
            )
            node["widgets_values"] = [
                _portable_widget_value(
                    class_type,
                    name,
                    snapshot.values.get(name),
                    live_definition,
                )
                for name in names
            ]
            node.pop(WORKFLOW_WIDGET_VALUES_KEY, None)
    return canonical


def _portable_widget_value(
    class_type: str,
    input_name: str,
    value: Any,
    definition: Mapping[str, Any],
) -> Any:
    """Return one authored value or an unset placeholder for local reconciliation."""

    field_spec = find_input_field_spec(definition, input_name)
    if is_unselected_picker_value(value, field_spec):
        return None
    return (
        value
        if should_store_authored_value(
            class_type,
            input_name,
            field_spec=field_spec,
        )
        else None
    )


def _attach_explicit_widget_identities(
    node: dict[str, Any],
    values: Mapping[str, Any],
    definition: Mapping[str, Any],
) -> None:
    """Persist live widget names beside values missing Comfy input identities."""

    raw_inputs = node.get("inputs")
    inputs = list(raw_inputs) if _is_sequence(raw_inputs) else []
    identified: set[str] = set()
    linked = _linked_widget_names(node)
    for entry in inputs:
        if not isinstance(entry, Mapping):
            continue
        name = _widget_input_name(entry)
        if name is not None:
            identified.add(name)
    for name in values:
        if name in identified or name in linked:
            continue
        inputs.append(
            {
                "name": name,
                "type": _field_type(find_input_field_spec(definition, name)),
                "widget": {"name": name},
            }
        )
    node["inputs"] = inputs


def _linked_widget_names(node: Mapping[str, Any]) -> set[str]:
    """Return widget identities whose runtime values arrive through links."""

    inputs = node.get("inputs")
    if not _is_sequence(inputs):
        return set()
    return {
        name
        for entry in inputs
        if isinstance(entry, Mapping) and entry.get("link") is not None
        if (name := _widget_input_name(entry)) is not None
    }


def _widget_input_name(entry: Mapping[str, Any]) -> str | None:
    """Return one serialized widget identity from an input entry."""

    widget = entry.get("widget")
    if not isinstance(widget, Mapping):
        return None
    widget_name = widget.get("name")
    input_name = entry.get("name")
    name = widget_name if isinstance(widget_name, str) else input_name
    return name.strip() if isinstance(name, str) and name.strip() else None


def _field_type(field_spec: Any) -> Any:
    """Return the Comfy input type paired with a synthesized widget identity."""

    return deepcopy(field_spec[0]) if _is_sequence(field_spec) and field_spec else "*"


def _normalize_explicit_values(values: Mapping[Any, Any]) -> dict[str, Any]:
    """Normalize an explicit widget map while rejecting invalid identities."""

    normalized: dict[str, Any] = {}
    for raw_name, value in values.items():
        if not isinstance(raw_name, str) or not raw_name.strip():
            raise WidgetSnapshotError("Widget snapshot contains an invalid input name")
        name = raw_name.strip()
        if name in normalized:
            raise WidgetSnapshotError(
                f"Widget snapshot contains duplicate input name '{name}'"
            )
        normalized[name] = value
    return normalized


def _decode_positional_values(
    names: Sequence[str],
    values: Sequence[Any],
    definition: Mapping[str, Any],
) -> dict[str, Any]:
    """Decode positional values only with identities from the same snapshot."""

    if not names and values:
        raise WidgetSnapshotError(
            "Positional widget values have no same-snapshot widget identities"
        )
    decoded: dict[str, Any] = {}
    value_index = 0
    for name in names:
        if name in decoded:
            raise WidgetSnapshotError(
                f"Workflow snapshot contains duplicate widget name '{name}'"
            )
        if value_index >= len(values):
            raise WidgetSnapshotError(
                f"Workflow snapshot is missing the value for widget '{name}'"
            )
        decoded[name] = values[value_index]
        value_index += 1
        field_spec = find_input_field_spec(definition, name)
        if _has_control_after_generate(field_spec) and value_index < len(values):
            if values[value_index] in _CONTROL_AFTER_GENERATE_VALUES:
                value_index += 1
    if value_index != len(values):
        raise WidgetSnapshotError(
            "Positional widget values cannot be associated with same-snapshot names"
        )
    return decoded


def _decode_positional_prefix(
    names: Sequence[str],
    values: Sequence[Any],
    definition: Mapping[str, Any],
) -> dict[str, Any]:
    """Decode exact semantic fields while leaving trailing UI-only widgets alone."""

    decoded: dict[str, Any] = {}
    value_index = 0
    for name in names:
        if value_index >= len(values):
            raise WidgetSnapshotError(
                f"Workflow snapshot is missing the value for stable field '{name}'"
            )
        decoded[name] = values[value_index]
        value_index += 1
        field_spec = find_input_field_spec(definition, name)
        if _has_control_after_generate(field_spec) and value_index < len(values):
            if values[value_index] in _CONTROL_AFTER_GENERATE_VALUES:
                value_index += 1
    return decoded


def _unique_names(values: Sequence[str]) -> list[str]:
    """Normalize exact field identities and reject duplicate stable names."""

    result: list[str] = []
    for value in values:
        name = value.strip()
        if not name:
            raise WidgetSnapshotError("Cube surface contains an invalid field identity")
        if name in result:
            raise WidgetSnapshotError(
                f"Cube surface contains duplicate field identity '{name}'"
            )
        result.append(name)
    return result


def _has_control_after_generate(field_spec: Any) -> bool:
    """Return whether a field owns a positional control companion."""

    return _field_metadata(field_spec).get("control_after_generate") is True


def _field_metadata(field_spec: Any) -> Mapping[str, Any]:
    """Return metadata from one Comfy input field specification."""

    if (
        _is_sequence(field_spec)
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return {}


def _is_sequence(value: object) -> TypeGuard[Sequence[Any]]:
    """Return whether a value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
