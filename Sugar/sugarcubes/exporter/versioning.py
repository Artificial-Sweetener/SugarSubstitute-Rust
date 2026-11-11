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
"""Versioning helpers for SugarCubes exports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from ..cube_model import (
    CubeDocument,
    looks_like_current_cube_payload,
    looks_like_legacy_cube_payload,
    migrate_legacy_payload,
)
from ..cube_model.version_policy import suggest_version as suggest_document_version


@dataclass(frozen=True)
class VersionSuggestion:
    """Describe the semantic-version bump suggested for an export."""

    bump: str
    suggested: str
    reason: str


def suggest_version(
    old_cube: Mapping[str, Any], new_cube: Mapping[str, Any]
) -> VersionSuggestion:
    """Suggest a semantic version based on interface, topology, and config drift."""

    suggestion = suggest_document_version(
        _coerce_document(old_cube), _coerce_document(new_cube)
    )
    return VersionSuggestion(
        bump=suggestion.bump,
        suggested=suggestion.suggested,
        reason=suggestion.reason,
    )


def _coerce_document(value: Mapping[str, Any]) -> CubeDocument:
    """Coerce a persisted payload into the canonical cube document type."""

    if looks_like_current_cube_payload(value):
        return CubeDocument.from_dict(value)
    if looks_like_legacy_cube_payload(value):
        return migrate_legacy_payload(value)
    return CubeDocument.from_dict(value)


def resolve_input_type(definitions: Any, class_type: str, port_name: Any) -> str:
    """Resolve a node input type from definitions."""

    if not isinstance(definitions, Mapping):
        return ""
    definition = definitions.get(class_type)
    if not isinstance(definition, Mapping):
        return ""
    inputs = definition.get("input")
    if not isinstance(inputs, Mapping):
        return ""
    resolved_port_name = port_name if isinstance(port_name, str) else None
    for section in ("required", "optional", "hidden"):
        section_map = inputs.get(section)
        if not isinstance(section_map, Mapping):
            continue
        if resolved_port_name and resolved_port_name in section_map:
            return _normalize_type_spec(section_map.get(resolved_port_name))
    return ""


def resolve_output_type_by_slot(
    definitions: Any, class_type: str, slot_index: Any
) -> str:
    """Resolve a node output type by slot index."""
    if not isinstance(definitions, Mapping):
        return ""
    definition = definitions.get(class_type)
    if not isinstance(definition, Mapping):
        return ""
    outputs = definition.get("output")
    if isinstance(outputs, Sequence):
        if len(outputs) == 1:
            return _normalize_type_spec(outputs[0])
        if isinstance(slot_index, int) and 0 <= slot_index < len(outputs):
            return _normalize_type_spec(outputs[slot_index])
    return ""


def _normalize_type_spec(value: Any) -> str:
    """Normalize a Comfy type spec into the exported string form."""

    if isinstance(value, (list, tuple)) and value:
        head = value[0]
        return str(head)
    if isinstance(value, str):
        return value
    return ""
