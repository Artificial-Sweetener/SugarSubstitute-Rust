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
"""Authored default portability policy for saved SugarCube documents."""

from __future__ import annotations

from typing import Any, Mapping, MutableMapping

from .document import CubeDocument
from .input_persistence import should_store_authored_value
from .surface_value_policy import volatile_surface_control_ids


def sanitize_authored_defaults_document(document: CubeDocument) -> CubeDocument:
    """Return a cube document with unshippable authored defaults removed."""

    payload = document.to_dict()
    sanitize_authored_defaults_payload(payload)
    return CubeDocument.from_dict(payload)


def sanitize_authored_defaults_payload(payload: MutableMapping[str, Any]) -> None:
    """Remove authored values that should not be stored in portable cube files."""

    _strip_unshippable_node_inputs(payload.get("implementation"))
    surface = payload.get("surface")
    flavors = payload.get("flavors")
    if not isinstance(surface, Mapping) or not isinstance(flavors, Mapping):
        return

    stripped_control_ids = _stripped_control_ids(surface.get("controls"))
    if not stripped_control_ids:
        return

    authored = flavors.get("authored")
    if not isinstance(authored, list):
        return
    for flavor in authored:
        if not isinstance(flavor, MutableMapping):
            continue
        values = flavor.get("values")
        if not isinstance(values, MutableMapping):
            continue
        for control_id in stripped_control_ids:
            values.pop(control_id, None)


def _strip_unshippable_node_inputs(implementation: Any) -> None:
    """Remove scalar local and volatile defaults while retaining graph bindings."""

    if not isinstance(implementation, Mapping):
        return
    nodes = implementation.get("nodes")
    if not isinstance(nodes, Mapping):
        return
    for node in nodes.values():
        if not isinstance(node, Mapping):
            continue
        class_type = node.get("class_type")
        inputs = node.get("inputs")
        if not isinstance(class_type, str) or not isinstance(inputs, MutableMapping):
            continue
        for input_name, value in list(inputs.items()):
            if not isinstance(input_name, str) or _contains_runtime_reference(value):
                continue
            if not should_store_authored_value(class_type, input_name):
                inputs.pop(input_name, None)


def _contains_runtime_reference(value: Any) -> bool:
    """Return whether a node input contains a serialized graph relationship."""

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


def should_strip_authored_default(class_type: str, input_name: str) -> bool:
    """Return whether authored defaults for one volatile control should be omitted."""

    return not should_store_authored_value(class_type, input_name)


def _stripped_control_ids(controls: Any) -> set[str]:
    """Return surface control ids whose authored defaults should be stripped."""

    stripped: set[str] = volatile_surface_control_ids(controls)
    if not isinstance(controls, list):
        return stripped
    for control in controls:
        if not isinstance(control, Mapping):
            continue
        class_type = control.get("class_type")
        input_name = control.get("input_name")
        control_id = control.get("control_id")
        if (
            isinstance(class_type, str)
            and isinstance(input_name, str)
            and isinstance(control_id, str)
            and should_strip_authored_default(class_type, input_name)
        ):
            stripped.add(control_id)
    return stripped
