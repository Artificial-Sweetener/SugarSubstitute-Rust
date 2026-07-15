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
"""Validate stable, name-addressed values selected for cube persistence."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any, TypeGuard

from ..cube_model.input_persistence import should_store_authored_value
from ..cube_model.picker_fields import (
    find_input_field_spec,
    is_picker_field_spec,
    picker_options,
)
from ..cube_model.widget_values import (
    WidgetSnapshotError,
    decode_workflow_widget_snapshot,
)


class PersistedValueError(ValueError):
    """Reject one incompatible value without reassigning it by position."""


def validate_named_node_inputs(
    *,
    node_id: object,
    class_type: str,
    inputs: Mapping[str, Any],
    definition: Mapping[str, Any],
) -> None:
    """Validate portable scalar values against the same named live inputs."""

    for input_name, value in inputs.items():
        if _contains_runtime_reference(value):
            continue
        if not should_store_authored_value(class_type, input_name):
            continue
        field_spec = find_input_field_spec(definition, input_name)
        reason = invalid_named_value_reason(value, field_spec)
        if reason is not None:
            _raise_value_error(
                node_id=node_id,
                class_type=class_type,
                input_name=input_name,
                value=value,
                reason=reason,
            )


def validate_subgraph_widget_values(
    subgraphs: Sequence[Mapping[str, Any]],
    definitions: Mapping[str, Any],
) -> None:
    """Validate subgraph values through names stored in the same snapshot."""

    for subgraph in subgraphs:
        nodes = subgraph.get("nodes")
        if not _is_sequence(nodes):
            continue
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type", node.get("class_type"))
            if not isinstance(class_type, str):
                continue
            definition = definitions.get(class_type)
            if not isinstance(definition, Mapping):
                continue
            try:
                snapshot = decode_workflow_widget_snapshot(node, definition)
            except WidgetSnapshotError as exc:
                raise PersistedValueError(
                    "Unsafe subgraph widget snapshot: "
                    f"node_id={node.get('id')!r}; class_type={class_type}; {exc}"
                ) from exc
            if snapshot is None:
                continue
            validate_named_node_inputs(
                node_id=node.get("id"),
                class_type=class_type,
                inputs=snapshot.values,
                definition=definition,
            )


def invalid_named_value_reason(value: Any, field_spec: Any) -> str | None:
    """Return why a portable value contradicts its same-named field."""

    if not _is_sequence(field_spec) or not field_spec:
        return None
    if is_picker_field_spec(field_spec):
        options = picker_options(field_spec)
        if not isinstance(value, str):
            return "is not a string picker value"
        if options and value not in options:
            return "is not an available stable choice"
        return None
    input_type = field_spec[0]
    normalized_type = input_type.upper() if isinstance(input_type, str) else ""
    if normalized_type == "BOOLEAN" and not isinstance(value, bool):
        return "is not a boolean"
    if normalized_type == "INT" and (
        isinstance(value, bool)
        or not isinstance(value, int | float)
        or int(value) != value
    ):
        return "is not an integer"
    if normalized_type in {"FLOAT", "NUMBER"} and (
        isinstance(value, bool) or not isinstance(value, int | float)
    ):
        return "is not a number"
    if normalized_type in {"STRING", "TEXT"} and not isinstance(value, str):
        return "is not a string"
    metadata = _field_metadata(field_spec)
    if isinstance(value, int | float) and not isinstance(value, bool):
        minimum = metadata.get("min")
        maximum = metadata.get("max")
        if isinstance(minimum, int | float) and value < minimum:
            return f"is below minimum {minimum}"
        if isinstance(maximum, int | float) and value > maximum:
            return f"is above maximum {maximum}"
    return None


def _raise_value_error(
    *,
    node_id: object,
    class_type: str,
    input_name: str,
    value: Any,
    reason: str,
) -> None:
    """Raise one diagnostic error for a stable input relationship."""

    raise PersistedValueError(
        "Unsafe cube value: "
        f"node_id={node_id!r}; class_type={class_type}; input={input_name}; "
        f"value={value!r} {reason}"
    )


def _contains_runtime_reference(value: Any) -> bool:
    """Return whether one input value contains a node or cube binding reference."""

    if isinstance(value, list):
        if (
            len(value) == 2
            and isinstance(value[0], str | int)
            and isinstance(value[1], str | int)
        ):
            return True
        return any(_contains_runtime_reference(entry) for entry in value)
    if isinstance(value, Mapping):
        return any(_contains_runtime_reference(entry) for entry in value.values())
    return False


def _field_metadata(field_spec: Any) -> Mapping[str, Any]:
    """Return metadata from one field specification."""

    if (
        _is_sequence(field_spec)
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return {}


def _is_sequence(value: object) -> TypeGuard[Sequence[Any]]:
    """Return whether one value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
