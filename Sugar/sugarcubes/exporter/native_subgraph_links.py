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
"""Validate serialized ComfyUI subgraph links at the exporter boundary."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping, Sequence, TypeAlias, TypeGuard

SerializedGraphId: TypeAlias = int | str
SerializedLinkId: TypeAlias = int | str


@dataclass(frozen=True)
class NativeSubgraphLink:
    """Describe one validated object- or tuple-shaped ComfyUI subgraph link."""

    link_id: SerializedLinkId
    origin_id: SerializedGraphId
    origin_slot: int
    target_id: SerializedGraphId
    target_slot: int
    link_type: object


def read_native_subgraph_links(
    value: object, *, cube_id: str
) -> tuple[NativeSubgraphLink, ...]:
    """Validate every serialized link so projection cannot skip malformed topology."""

    if value is None:
        return ()
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"Native Cube '{cube_id}' has invalid serialized links")
    return tuple(
        _read_native_subgraph_link(entry, cube_id=cube_id, index=index)
        for index, entry in enumerate(value)
    )


def _read_native_subgraph_link(
    value: object, *, cube_id: str, index: int
) -> NativeSubgraphLink:
    """Validate one serialized link with an actionable Cube-specific error."""

    if isinstance(value, Mapping):
        link_id = value.get("id")
        origin_id = value.get("origin_id")
        origin_slot = value.get("origin_slot")
        target_id = value.get("target_id")
        target_slot = value.get("target_slot")
        link_type = value.get("type", "*")
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        if len(value) < 5:
            raise _invalid_link_error(cube_id, index)
        link_id = value[0]
        origin_id = value[1]
        origin_slot = value[2]
        target_id = value[3]
        target_slot = value[4]
        link_type = value[5] if len(value) > 5 else "*"
    else:
        raise _invalid_link_error(cube_id, index)

    parsed_link_id = read_serialized_graph_id(link_id)
    parsed_origin_id = read_serialized_graph_id(origin_id)
    parsed_target_id = read_serialized_graph_id(target_id)
    if parsed_link_id is None:
        raise _invalid_link_error(cube_id, index)
    if parsed_origin_id is None or parsed_target_id is None:
        raise _invalid_link_error(cube_id, index)
    if not _is_slot(origin_slot) or not _is_slot(target_slot):
        raise _invalid_link_error(cube_id, index)
    return NativeSubgraphLink(
        link_id=parsed_link_id,
        origin_id=parsed_origin_id,
        origin_slot=origin_slot,
        target_id=parsed_target_id,
        target_slot=target_slot,
        link_type=link_type,
    )


def read_serialized_graph_id(value: object) -> SerializedGraphId | None:
    """Read one exact number-or-string ComfyUI graph identity."""

    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, str) and value:
        return value
    return None


def _is_slot(value: object) -> TypeGuard[int]:
    """Return whether a dynamic value is a valid non-negative slot index."""

    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _invalid_link_error(cube_id: str, index: int) -> ValueError:
    """Build the canonical error for malformed host link data."""

    return ValueError(
        f"Native Cube '{cube_id}' has an invalid serialized link at index {index}"
    )
