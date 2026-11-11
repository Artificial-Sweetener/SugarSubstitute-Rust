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
"""Validate and copy derived prompt values used during inheritance."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping, MutableMapping, Sequence, cast

from .errors import CubeInheritanceError


def copy_prompt(
    prompt: Mapping[str, Mapping[str, object]],
) -> dict[str, dict[str, object]]:
    """Deep-copy the derived prompt before inheritance writes."""

    return {node_id: deepcopy(dict(node)) for node_id, node in prompt.items()}


def mutable_inputs(
    node: MutableMapping[str, object], node_id: str
) -> MutableMapping[str, object]:
    """Return one lowered input object or fail on an invalid internal result."""

    inputs = node.get("inputs")
    if not isinstance(inputs, MutableMapping):
        raise CubeInheritanceError(
            "execution.inheritance.invalid_inputs",
            f"Lowered node '{node_id}' has no input object.",
            target_instance_id="",
            slot="",
        )
    return cast(MutableMapping[str, object], inputs)


def connection(value: object) -> tuple[str, int] | None:
    """Recognize one execution prompt connection tuple."""

    if (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes, bytearray))
        and len(value) == 2
        and isinstance(value[0], str)
        and isinstance(value[1], int)
        and not isinstance(value[1], bool)
    ):
        return value[0], value[1]
    return None


def selector_input(name: str) -> bool:
    """Return whether an input name encodes authored selector priority."""

    normalized = name.lower()
    return normalized.isdigit() or (
        normalized.startswith("any_") and normalized.removeprefix("any_").isdigit()
    )
