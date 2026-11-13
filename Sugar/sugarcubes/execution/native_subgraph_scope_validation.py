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
"""Validate bounded native subgraph scopes before execution lowering."""

from __future__ import annotations

from collections.abc import Mapping

from .errors import CubeLoweringError
from .limits import (
    MAX_NATIVE_SCOPE_DEPTH,
    MAX_NATIVE_SCOPE_LINKS,
    MAX_NATIVE_SCOPE_NODES,
)
from .native_subgraph_values import sequence


def validate_native_scope(
    payload: Mapping[str, object], ancestry: tuple[str, ...]
) -> None:
    """Reject hostile native breadth or depth before indexing graph values."""

    if len(ancestry) > MAX_NATIVE_SCOPE_DEPTH:
        raise CubeLoweringError(
            "execution.lowering.subgraph_depth_exceeded",
            f"Native subgraph nesting exceeds {MAX_NATIVE_SCOPE_DEPTH} scopes.",
        )
    for values, maximum, label in (
        (
            sequence(payload.get("nodes"), "subgraph nodes"),
            MAX_NATIVE_SCOPE_NODES,
            "nodes",
        ),
        (
            sequence(payload.get("links"), "subgraph links"),
            MAX_NATIVE_SCOPE_LINKS,
            "links",
        ),
    ):
        if len(values) > maximum:
            raise CubeLoweringError(
                "execution.lowering.subgraph_limit_exceeded",
                f"Native subgraph {label} exceed the limit of {maximum} entries.",
            )


def native_scope_identity(payload: Mapping[str, object], fallback: str) -> str:
    """Read one scope identity without trusting malformed host identifiers."""

    value = payload.get("id")
    return value if isinstance(value, str) and value else fallback
