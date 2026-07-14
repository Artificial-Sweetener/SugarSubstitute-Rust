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
"""Picker field semantics shared by cube export, repair, and consumers."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

PICKER_SENTINEL = "LIST"

_OBJECT_INFO_INPUT_SECTIONS = ("required", "optional")
_WIDGET_INPUT_TYPES = frozenset(
    {
        "BOOLEAN",
        "COMBO",
        "FLOAT",
        "INT",
        "LIST",
        "NUMBER",
        "STRING",
        "TEXT",
    }
)


@dataclass(frozen=True)
class PickerFallback:
    """Describe the local default value for one picker input."""

    value: Any
    source: str


def compact_picker_field_spec(field_spec: Any) -> Any:
    """Return the compact cube representation for one picker field spec."""

    if is_picker_field_spec(field_spec):
        return [PICKER_SENTINEL]
    return field_spec


def is_picker_field_spec(field_spec: Any) -> bool:
    """Return whether a Comfy input field spec is a list or combo picker."""

    if not isinstance(field_spec, Sequence) or isinstance(field_spec, (str, bytes)):
        return False
    if not field_spec:
        return False
    first = field_spec[0]
    if first == PICKER_SENTINEL:
        return True
    if isinstance(first, Sequence) and not isinstance(first, (str, bytes)):
        return True
    return (
        isinstance(first, str)
        and first.upper() == "COMBO"
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
        and isinstance(field_spec[1].get("options"), Sequence)
        and not isinstance(field_spec[1].get("options"), (str, bytes))
    )


def picker_options(field_spec: Any) -> list[Any]:
    """Return local picker options from a classic list or new API combo spec."""

    if not isinstance(field_spec, Sequence) or isinstance(field_spec, (str, bytes)):
        return []
    if not field_spec:
        return []
    first = field_spec[0]
    if isinstance(first, Sequence) and not isinstance(first, (str, bytes)):
        return list(first)
    if (
        isinstance(first, str)
        and first.upper() == "COMBO"
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        options = field_spec[1].get("options")
        if isinstance(options, Sequence) and not isinstance(options, (str, bytes)):
            return list(options)
    return []


def resolve_picker_fallback(field_spec: Any) -> PickerFallback | None:
    """Resolve the local default or first option for one picker field spec."""

    options = picker_options(field_spec)
    metadata = _field_metadata(field_spec)
    if metadata and "default" in metadata:
        default_value = metadata["default"]
        if not options or default_value in options:
            return PickerFallback(value=default_value, source="default")
    if options:
        return PickerFallback(value=options[0], source="first_option")
    return None


def find_input_field_spec(
    definition: Mapping[str, Any],
    input_name: str,
) -> Any | None:
    """Return the object-info field spec for one input name when available."""

    inputs = definition.get("input")
    if not isinstance(inputs, Mapping):
        return None
    for section_name in _ordered_input_sections(inputs):
        section = inputs.get(section_name)
        if isinstance(section, Mapping) and input_name in section:
            return section[input_name]
    return None


def widget_input_names(definition: Mapping[str, Any]) -> list[str]:
    """Return widget-backed input names in Comfy workflow widget order."""

    inputs = definition.get("input")
    if not isinstance(inputs, Mapping):
        return []
    ordered_names: list[str] = []
    for section_name in _ordered_input_sections(inputs):
        section = inputs.get(section_name)
        if not isinstance(section, Mapping):
            continue
        for input_name in _ordered_section_names(definition, section_name, section):
            field_spec = section.get(input_name)
            if is_widget_field_spec(field_spec):
                ordered_names.append(str(input_name))
    return ordered_names


def is_widget_field_spec(field_spec: Any) -> bool:
    """Return whether a field spec is normally represented as a widget value."""

    metadata = _field_metadata(field_spec)
    if metadata and metadata.get("forceInput") is True:
        return False
    if is_picker_field_spec(field_spec):
        return True
    if not isinstance(field_spec, Sequence) or isinstance(field_spec, (str, bytes)):
        return False
    if not field_spec:
        return False
    first = field_spec[0]
    return isinstance(first, str) and first.upper() in _WIDGET_INPUT_TYPES


def _field_metadata(field_spec: Any) -> Mapping[str, Any] | None:
    """Return the metadata mapping from one field spec when present."""

    if (
        isinstance(field_spec, Sequence)
        and not isinstance(field_spec, (str, bytes))
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return None


def _ordered_input_sections(inputs: Mapping[str, Any]) -> list[str]:
    """Return input section names with known Comfy sections first."""

    sections: list[str] = []
    for section_name in _OBJECT_INFO_INPUT_SECTIONS:
        if section_name in inputs:
            sections.append(section_name)
    for section_name in inputs:
        if section_name not in sections:
            sections.append(str(section_name))
    return sections


def _ordered_section_names(
    definition: Mapping[str, Any],
    section_name: str,
    section: Mapping[str, Any],
) -> list[str]:
    """Return input names in object-info order for one section."""

    input_order = definition.get("input_order")
    if isinstance(input_order, Mapping):
        section_order = input_order.get(section_name)
        if isinstance(section_order, Sequence) and not isinstance(
            section_order, (str, bytes)
        ):
            ordered = [str(name) for name in section_order if str(name) in section]
            ordered.extend(str(name) for name in section if str(name) not in ordered)
            return ordered
    return [str(name) for name in section]
