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
"""Own the connections a stack of Cubes implies by its order alone.

SugarSubstitute shows a workflow as a stack: each Cube works on what the Cubes above it made.
Neither its workflow files nor its project scripts state those connections; they follow from
the order. A Cube input named `input.<x>` is fed by the nearest earlier Cube that offers an
output named `output.<x>`.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from ..language.compiler_models import CompiledCubeConnection

_INPUT_PREFIX = "input."
_OUTPUT_PREFIX = "output."


def stack_connections(
    stack: Sequence[tuple[str, Mapping[str, object]]],
) -> tuple[CompiledCubeConnection, ...]:
    """Return the connections implied by Cubes given in stack order.

    Args:
        stack: Each Cube's instance identity and its Cube document, first Cube first.
    """

    connections: list[CompiledCubeConnection] = []
    for index, (target_id, document) in enumerate(stack):
        for name in _boundary_names(document, "inputs"):
            if not name.startswith(_INPUT_PREFIX):
                continue
            wanted = _OUTPUT_PREFIX + name[len(_INPUT_PREFIX) :]
            for source_id, source_document in reversed(stack[:index]):
                if wanted in _boundary_names(source_document, "outputs"):
                    connections.append(
                        CompiledCubeConnection(source_id, wanted, target_id, name)
                    )
                    break
    return tuple(connections)


def _boundary_names(document: Mapping[str, object], key: str) -> tuple[str, ...]:
    """Read a document's public input or output names in authored order."""

    implementation = document.get("implementation")
    boundary = implementation.get(key) if isinstance(implementation, Mapping) else None
    return tuple(boundary) if isinstance(boundary, Mapping) else ()


__all__ = ["stack_connections"]
