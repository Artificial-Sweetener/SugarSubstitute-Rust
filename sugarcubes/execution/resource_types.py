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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Resolve inheritable resource types from embedded Comfy definitions."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from .models import InheritanceSlot, LoweringResult


def input_slot(
    node_id: str,
    input_name: str,
    lowered: LoweringResult,
) -> InheritanceSlot | None:
    """Resolve an inheritable resource without overriding declared input types."""

    definition = lowered.node_definitions.get(node_id, {})
    envelope = definition.get("input")
    if isinstance(envelope, Mapping):
        for section_name in ("required", "optional", "hidden"):
            section = envelope.get(section_name)
            if isinstance(section, Mapping) and input_name in section:
                return normalize_slot(field_type(section[input_name]))
    normalized = input_name.lower()
    direct = normalize_slot(normalized)
    if direct is not None:
        return direct
    if normalized.endswith("_in"):
        wrapped = normalize_slot(normalized[:-3])
        if wrapped is not None:
            return wrapped
    return None


def output_slots(
    definition: Mapping[str, object],
) -> tuple[InheritanceSlot | str | None, ...]:
    """Return normalized declared output types in stable slot order."""

    output = definition.get("output")
    if not isinstance(output, Sequence) or isinstance(output, (str, bytes, bytearray)):
        return ()
    return tuple(_normalize_output(item) for item in output)


def output_slot(
    definition: Mapping[str, object], output_index: int
) -> InheritanceSlot | str | None:
    """Return one declared output type or output-name fallback."""

    slots = output_slots(definition)
    if 0 <= output_index < len(slots):
        return slots[output_index]
    names = definition.get("output_name")
    if isinstance(names, Sequence) and not isinstance(names, (str, bytes, bytearray)):
        if 0 <= output_index < len(names):
            return _normalize_output(names[output_index])
    return None


def normalize_slot(value: object) -> InheritanceSlot | None:
    """Narrow one exact resource type to the inheritance slot union."""

    if not isinstance(value, str):
        return None
    normalized = value.strip().lower()
    if normalized == "model":
        return "model"
    if normalized == "clip":
        return "clip"
    if normalized == "vae":
        return "vae"
    return None


def field_type(value: object) -> str | None:
    """Read one compact Comfy field declaration."""

    if isinstance(value, str):
        return value
    if (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes, bytearray))
        and value
    ):
        return value[0] if isinstance(value[0], str) else None
    return None


def _normalize_output(value: object) -> InheritanceSlot | str | None:
    """Normalize one output declaration without broadening unknown types."""

    if not isinstance(value, str):
        return None
    normalized = value.strip().lower()
    slot = normalize_slot(normalized)
    return slot if slot is not None else normalized.upper()
