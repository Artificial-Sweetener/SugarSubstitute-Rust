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
"""Validate positional Comfy widget data before exporting a cube."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

try:
    from ..cube_model.picker_fields import (
        find_input_field_spec,
        is_picker_field_spec,
        picker_options,
        widget_input_names,
    )
except ImportError:
    from cube_model.picker_fields import (
        find_input_field_spec,
        is_picker_field_spec,
        picker_options,
        widget_input_names,
    )

_CONTROL_AFTER_GENERATE_VALUES = frozenset(
    {"fixed", "increment", "decrement", "randomize"}
)


class WidgetSaveValidationError(ValueError):
    """Reject cube exports whose positional widget data is not trustworthy."""


def serialized_widget_names(node: Mapping[str, object]) -> list[str]:
    """Return widget input names from a serialized Comfy workflow node."""

    inputs = node.get("inputs")
    if not _is_sequence(inputs):
        return []
    names: list[str] = []
    for entry in inputs:
        if not isinstance(entry, Mapping):
            continue
        widget = entry.get("widget")
        if not isinstance(widget, Mapping):
            continue
        widget_name = widget.get("name")
        input_name = entry.get("name")
        if isinstance(widget_name, str) and widget_name:
            names.append(widget_name)
        elif isinstance(input_name, str) and input_name:
            names.append(input_name)
    return names


def validate_subgraph_widget_values(
    subgraphs: Sequence[Mapping[str, object]],
    definitions: Mapping[str, object],
) -> None:
    """Validate every concrete node embedded in exported subgraph definitions."""

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
            widget_values = node.get("widgets_values")
            if not _is_sequence(widget_values):
                continue
            validate_serialized_widget_values(
                node_id=node.get("id"),
                class_type=class_type,
                persisted_widget_names=serialized_widget_names(node),
                widget_values=widget_values,
                live_definition=definition,
            )


def validate_serialized_widget_values(
    *,
    node_id: object,
    class_type: str,
    persisted_widget_names: Sequence[str],
    widget_values: Sequence[object],
    live_definition: Mapping[str, object],
) -> None:
    """Require one positional widget array to match the current Comfy contract."""

    live_names = widget_input_names(live_definition)
    persisted_names = list(persisted_widget_names)
    if persisted_names != live_names:
        _raise_validation_error(
            node_id=node_id,
            class_type=class_type,
            input_name=None,
            reason=(
                "persisted widget order does not match live Comfy widget order; "
                f"persisted={persisted_names!r}; live={live_names!r}"
            ),
        )
    value_index = 0
    for input_name in live_names:
        if value_index >= len(widget_values):
            _raise_validation_error(
                node_id=node_id,
                class_type=class_type,
                input_name=input_name,
                reason="persisted widget value is missing",
            )
        field_spec = find_input_field_spec(live_definition, input_name)
        value = widget_values[value_index]
        reason = invalid_widget_value_reason(value, field_spec)
        if reason is not None:
            _raise_validation_error(
                node_id=node_id,
                class_type=class_type,
                input_name=input_name,
                reason=f"value {value!r} {reason}",
            )
        value_index += 1
        if _has_control_after_generate(field_spec):
            if value_index >= len(widget_values):
                _raise_validation_error(
                    node_id=node_id,
                    class_type=class_type,
                    input_name=input_name,
                    reason="control-after-generate value is missing",
                )
            control = widget_values[value_index]
            if control not in _CONTROL_AFTER_GENERATE_VALUES:
                _raise_validation_error(
                    node_id=node_id,
                    class_type=class_type,
                    input_name=input_name,
                    reason=f"control-after-generate value {control!r} is invalid",
                )
            value_index += 1
    if value_index != len(widget_values):
        _raise_validation_error(
            node_id=node_id,
            class_type=class_type,
            input_name=None,
            reason=(
                f"{len(widget_values) - value_index} unassociated positional "
                "widget value(s) remain"
            ),
        )


def invalid_widget_value_reason(value: object, field_spec: object) -> str | None:
    """Return why a value violates a live Comfy field definition, if it does."""

    if not _is_sequence(field_spec) or not field_spec:
        return None
    if is_picker_field_spec(field_spec):
        options = picker_options(field_spec)
        if options and value not in options:
            return "is not an available choice"
        return None
    input_type = field_spec[0]
    normalized_type = input_type.upper() if isinstance(input_type, str) else ""
    if normalized_type == "BOOLEAN":
        if not isinstance(value, bool):
            return "is not a boolean"
    elif normalized_type == "INT":
        if (
            isinstance(value, bool)
            or not isinstance(value, int | float)
            or int(value) != value
        ):
            return "is not an integer"
    elif normalized_type in {"FLOAT", "NUMBER"}:
        if isinstance(value, bool) or not isinstance(value, int | float):
            return "is not a number"
    elif normalized_type in {"STRING", "TEXT"}:
        if not isinstance(value, str):
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


def _has_control_after_generate(field_spec: object) -> bool:
    """Return whether Comfy declares a serialized control companion."""

    return _field_metadata(field_spec).get("control_after_generate") is True


def _field_metadata(field_spec: object) -> Mapping[str, object]:
    """Return metadata from one Comfy field specification."""

    if (
        _is_sequence(field_spec)
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return {}


def _raise_validation_error(
    *,
    node_id: object,
    class_type: str,
    input_name: str | None,
    reason: str,
) -> None:
    """Raise one consistently structured save-time validation failure."""

    input_context = f"; input={input_name}" if input_name is not None else ""
    raise WidgetSaveValidationError(
        "Unsafe cube widget serialization: "
        f"node_id={node_id!r}; class_type={class_type}{input_context}; {reason}"
    )


def _is_sequence(value: object) -> bool:
    """Return whether a JSON value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
