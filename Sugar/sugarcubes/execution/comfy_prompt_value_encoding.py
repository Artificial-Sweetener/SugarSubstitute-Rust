#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Encode atomic widget lists for Comfy's link-ambiguous prompt protocol."""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping, Sequence
from copy import deepcopy

from ..cube_model.picker_fields import find_input_field_spec, is_widget_field_spec


def encode_atomic_list_widget_values(
    prompt: MutableMapping[str, dict[str, object]],
    node_definitions: Mapping[str, Mapping[str, object]],
) -> None:
    """Wrap list-valued widgets while preserving genuine prompt connections."""

    for node_id, node in prompt.items():
        inputs = node.get("inputs")
        definition = node_definitions.get(node_id)
        if not isinstance(inputs, MutableMapping) or not isinstance(
            definition, Mapping
        ):
            continue
        for raw_name, value in tuple(inputs.items()):
            name = str(raw_name)
            if not isinstance(value, list) or _is_prompt_link(value, prompt):
                continue
            field_spec = find_input_field_spec(definition, name)
            if is_widget_field_spec(field_spec):
                inputs[name] = {"__value__": deepcopy(value)}


def _is_prompt_link(value: Sequence[object], prompt: Mapping[str, object]) -> bool:
    """Recognize only a resolvable Comfy ``[node_id, output_index]`` link."""

    if len(value) != 2:
        return False
    origin, output_index = value
    if not isinstance(origin, str | int) or isinstance(origin, bool):
        return False
    return (
        isinstance(output_index, int)
        and not isinstance(output_index, bool)
        and str(origin) in prompt
    )
