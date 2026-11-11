#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Persist compiled instance values through stable Cube surface controls."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from typing import Any


def persist_instance_control_values(
    payload: dict[str, Any],
    *,
    authored_flavor_id: str,
) -> None:
    """Make the selected flavor reproduce the compiled implementation values."""

    surface = _required_mapping(payload.get("surface"), "surface")
    implementation = _required_mapping(payload.get("implementation"), "implementation")
    nodes = _required_mapping(implementation.get("nodes"), "implementation.nodes")
    flavors = _required_mapping(payload.get("flavors"), "flavors")
    authored = _required_records(flavors.get("authored"), "flavors.authored")
    controls = _required_records(surface.get("controls"), "surface.controls")
    selected = next(
        (flavor for flavor in authored if flavor.get("id") == authored_flavor_id),
        None,
    )
    if selected is None:
        raise ValueError(f"Authored flavor '{authored_flavor_id}' does not exist.")
    values = selected.get("values")
    if not isinstance(values, dict):
        raise ValueError(f"Authored flavor '{authored_flavor_id}' has invalid values.")
    surface["default_flavor_id"] = authored_flavor_id
    for control in controls:
        control_id = control.get("control_id")
        symbol = control.get("symbol")
        input_name = control.get("input_name")
        if (
            not isinstance(control_id, str)
            or not isinstance(symbol, str)
            or not isinstance(input_name, str)
        ):
            raise ValueError("Cube surface control identity is invalid.")
        node = nodes.get(symbol)
        inputs = node.get("inputs") if isinstance(node, Mapping) else None
        if isinstance(inputs, Mapping) and input_name in inputs:
            values[control_id] = deepcopy(inputs[input_name])


def _required_mapping(value: object, label: str) -> dict[str, Any]:
    """Require one mutable string-keyed document mapping."""

    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ValueError(f"Cube {label} is invalid.")
    return value


def _required_records(value: object, label: str) -> list[dict[str, Any]]:
    """Require one mutable sequence of document records."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ValueError(f"Cube {label} is invalid.")
    records = list(value)
    if not all(isinstance(item, dict) for item in records):
        raise ValueError(f"Cube {label} is invalid.")
    return records


__all__ = ["persist_instance_control_values"]
