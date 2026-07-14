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
"""Resolve Comfy node inputs without retaining positional widget identity."""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping, Sequence
from typing import Any

try:
    from ..cube_model.picker_fields import (
        find_input_field_spec,
        is_picker_field_spec,
        widget_input_names,
    )
except ImportError:
    from cube_model.picker_fields import (
        find_input_field_spec,
        is_picker_field_spec,
        widget_input_names,
    )

from .graph import GraphNode

try:
    from ..cube_model.widget_values import (
        WidgetSnapshotError,
        decode_workflow_widget_snapshot,
    )
except ImportError:
    from cube_model.widget_values import (
        WidgetSnapshotError,
        decode_workflow_widget_snapshot,
    )


def backfill_missing_widget_inputs(
    payload_inputs: MutableMapping[str, Any],
    node: GraphNode,
    workflow_node: Mapping[str, Any] | None,
    definition: Mapping[str, Any] | None,
) -> None:
    """Backfill only named widget values absent from the prompt payload."""

    if not isinstance(workflow_node, Mapping) or not isinstance(definition, Mapping):
        return
    missing_names = {
        input_name
        for input_name in widget_input_names(definition)
        if input_name not in payload_inputs
    }
    if not missing_names:
        return
    if (
        "widgets_values" not in workflow_node
        and "sugarcubes_widget_values" not in workflow_node
    ):
        return
    try:
        snapshot = decode_workflow_widget_snapshot(workflow_node, definition)
    except WidgetSnapshotError as exc:
        raise ValueError(
            "Unsafe workflow widget snapshot: "
            f"node_id={node.id}; class_type={node.class_type}; {exc}"
        ) from exc
    if snapshot is None:
        return
    for input_name, value in snapshot.values.items():
        if input_name not in missing_names:
            continue
        field_spec = find_input_field_spec(definition, input_name)
        compatible, coerced_value = _coerce_widget_value(value, field_spec)
        if not compatible:
            raise ValueError(
                "Unsafe workflow widget snapshot: value is incompatible with "
                f"its named input; node_id={node.id}; class_type={node.class_type}; "
                f"input={input_name}; actual_type={type(value).__name__}; "
                f"actual_value={value!r}"
            )
        payload_inputs[input_name] = _json_value(coerced_value)


def _coerce_widget_value(value: Any, field_spec: Any) -> tuple[bool, Any]:
    """Return whether a named workflow value matches its coarse field type."""

    if is_picker_field_spec(field_spec):
        return (True, value) if isinstance(value, str) else (False, None)
    input_type = _field_type_name(field_spec)
    if input_type is None:
        return True, value
    normalized_type = input_type.upper()
    if normalized_type == "BOOLEAN":
        return (True, value) if isinstance(value, bool) else (False, None)
    if normalized_type == "INT":
        if isinstance(value, bool):
            return False, None
        coerced = _coerce_int(value)
        return (True, coerced) if coerced is not None else (False, None)
    if normalized_type in {"FLOAT", "NUMBER"}:
        if isinstance(value, bool):
            return False, None
        coerced_float = _coerce_float(value)
        return (True, coerced_float) if coerced_float is not None else (False, None)
    if normalized_type in {"STRING", "TEXT"}:
        return (True, value) if isinstance(value, str) else (False, None)
    return True, value


def _field_type_name(field_spec: Any) -> str | None:
    """Return the declared scalar type for one field specification."""

    if isinstance(field_spec, str):
        return field_spec
    if not _is_sequence(field_spec) or not field_spec:
        return None
    return field_spec[0] if isinstance(field_spec[0], str) else None


def _coerce_int(value: Any) -> int | None:
    """Coerce a safely integral workflow value."""

    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    if isinstance(value, str):
        try:
            parsed = float(value.strip())
        except ValueError:
            return None
        return int(parsed) if parsed.is_integer() else None
    return None


def _coerce_float(value: Any) -> float | None:
    """Coerce one numeric workflow value without inventing a fallback."""

    if isinstance(value, int | float):
        return float(value)
    if isinstance(value, str) and value.strip():
        try:
            return float(value.strip())
        except ValueError:
            return None
    return None


def _json_value(value: Any) -> Any:
    """Convert nested tuples and mappings into JSON-ready containers."""

    if isinstance(value, Mapping):
        return {str(key): _json_value(entry) for key, entry in value.items()}
    if isinstance(value, tuple | list):
        return [_json_value(entry) for entry in value]
    return value


def _is_sequence(value: Any) -> bool:
    """Return whether a value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
