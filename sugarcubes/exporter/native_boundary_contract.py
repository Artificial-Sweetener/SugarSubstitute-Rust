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
"""Enforce the native ComfyUI interface contract before Cube projection."""

from __future__ import annotations

from collections import defaultdict
from typing import Mapping, Sequence

from .native_subgraph_links import (
    NativeSubgraphLink,
    SerializedGraphId,
    SerializedLinkId,
)


def validate_native_boundary_contract(
    definition: Mapping[str, object],
    links: Sequence[NativeSubgraphLink],
    *,
    body_nodes: Mapping[SerializedGraphId, Mapping[str, object]],
    cube_id: str,
) -> None:
    """Reject interface and topology mismatches that would amputate Cube ports."""

    input_node_id = _io_node_id(definition.get("inputNode"), -10)
    output_node_id = _io_node_id(definition.get("outputNode"), -20)
    inputs = _read_interface(definition.get("inputs"), cube_id, "input")
    outputs = _read_interface(definition.get("outputs"), cube_id, "output")
    actual_inputs: dict[int, list[SerializedLinkId]] = defaultdict(list)
    actual_outputs: dict[int, list[SerializedLinkId]] = defaultdict(list)
    missing_body_endpoint = False

    for link in links:
        if _same_id(link.origin_id, input_node_id):
            target_node = body_nodes.get(link.target_id)
            if target_node is None:
                raise _dangling_boundary_error(cube_id, "input", link.origin_slot)
            _require_interface_slot(inputs, link.origin_slot, cube_id, "input")
            _validate_target_endpoint(
                target_node,
                node_id=link.target_id,
                slot=link.target_slot,
                link_id=link.link_id,
                cube_id=cube_id,
            )
            actual_inputs[link.origin_slot].append(link.link_id)
            continue
        if _same_id(link.target_id, output_node_id):
            origin_node = body_nodes.get(link.origin_id)
            if origin_node is None:
                raise _dangling_boundary_error(cube_id, "output", link.target_slot)
            _require_interface_slot(outputs, link.target_slot, cube_id, "output")
            _validate_source_endpoint(
                origin_node,
                node_id=link.origin_id,
                slot=link.origin_slot,
                link_id=link.link_id,
                cube_id=cube_id,
            )
            actual_outputs[link.target_slot].append(link.link_id)
            continue
        if _same_id(link.target_id, input_node_id):
            raise ValueError(
                f"Native Cube '{cube_id}' has a link targeting its input boundary"
            )
        if _same_id(link.origin_id, output_node_id):
            raise ValueError(
                f"Native Cube '{cube_id}' has a link originating from its output boundary"
            )
        origin_node = body_nodes.get(link.origin_id)
        target_node = body_nodes.get(link.target_id)
        if origin_node is None or target_node is None:
            missing_body_endpoint = True
            continue
        _validate_source_endpoint(
            origin_node,
            node_id=link.origin_id,
            slot=link.origin_slot,
            link_id=link.link_id,
            cube_id=cube_id,
        )
        _validate_target_endpoint(
            target_node,
            node_id=link.target_id,
            slot=link.target_slot,
            link_id=link.link_id,
            cube_id=cube_id,
        )

    _validate_interface_links(inputs, actual_inputs, cube_id=cube_id, direction="input")
    _validate_interface_links(
        outputs, actual_outputs, cube_id=cube_id, direction="output"
    )
    for slot, link_ids in actual_outputs.items():
        if len(link_ids) > 1:
            raise ValueError(
                f"Native Cube '{cube_id}' output slot {slot} has multiple sources"
            )
    if missing_body_endpoint:
        raise ValueError(
            f"Native Cube '{cube_id}' has a link with a missing body-node endpoint"
        )


def _validate_source_endpoint(
    node: Mapping[str, object],
    *,
    node_id: SerializedGraphId,
    slot: int,
    link_id: SerializedLinkId,
    cube_id: str,
) -> None:
    """Require one source slot to exist and own the serialized link exactly once."""

    outputs = node.get("outputs")
    if not isinstance(outputs, Sequence) or isinstance(outputs, (str, bytes)):
        raise _invalid_endpoint_error(cube_id, "source", node_id, "output", slot)
    if slot >= len(outputs) or not isinstance(outputs[slot], Mapping):
        raise _invalid_endpoint_error(cube_id, "source", node_id, "output", slot)
    endpoint_links = outputs[slot].get("links")
    if not isinstance(endpoint_links, Sequence) or isinstance(
        endpoint_links, (str, bytes)
    ):
        raise _invalid_endpoint_error(cube_id, "source", node_id, "output", slot)
    matching_links = sum(
        1 for endpoint_link in endpoint_links if _same_link_id(endpoint_link, link_id)
    )
    if matching_links != 1:
        raise _invalid_endpoint_error(cube_id, "source", node_id, "output", slot)


def _validate_target_endpoint(
    node: Mapping[str, object],
    *,
    node_id: SerializedGraphId,
    slot: int,
    link_id: SerializedLinkId,
    cube_id: str,
) -> None:
    """Require one target slot to exist and own the serialized link identity."""

    inputs = node.get("inputs")
    if not isinstance(inputs, Sequence) or isinstance(inputs, (str, bytes)):
        raise _invalid_endpoint_error(cube_id, "target", node_id, "input", slot)
    if slot >= len(inputs) or not isinstance(inputs[slot], Mapping):
        raise _invalid_endpoint_error(cube_id, "target", node_id, "input", slot)
    if not _same_link_id(inputs[slot].get("link"), link_id):
        raise _invalid_endpoint_error(cube_id, "target", node_id, "input", slot)


def _read_interface(
    value: object, cube_id: str, direction: str
) -> tuple[Mapping[str, object], ...]:
    """Validate one native interface array without interpreting its display fields."""

    if value is None:
        return ()
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(
            f"Native Cube '{cube_id}' has an invalid {direction} interface"
        )
    entries: list[Mapping[str, object]] = []
    for slot, entry in enumerate(value):
        if not isinstance(entry, Mapping):
            raise ValueError(
                f"Native Cube '{cube_id}' has an invalid declared {direction} slot {slot}"
            )
        entries.append(entry)
    return tuple(entries)


def _require_interface_slot(
    interface: Sequence[Mapping[str, object]],
    slot: int,
    cube_id: str,
    direction: str,
) -> None:
    """Require a boundary link to address a declared interface slot."""

    if slot >= len(interface):
        raise _dangling_boundary_error(cube_id, direction, slot)


def _validate_interface_links(
    interface: Sequence[Mapping[str, object]],
    actual: Mapping[int, Sequence[SerializedLinkId]],
    *,
    cube_id: str,
    direction: str,
) -> None:
    """Compare current linkIds metadata with the links that define each boundary."""

    for slot, entry in enumerate(interface):
        if "linkIds" not in entry:
            continue
        declared = _read_link_ids(entry.get("linkIds"), cube_id, direction, slot)
        observed = tuple(actual.get(slot, ()))
        if set(declared) != set(observed) or len(declared) != len(observed):
            raise ValueError(
                f"Native Cube '{cube_id}' declared {direction} slot {slot} "
                "does not match its serialized boundary links"
            )


def _read_link_ids(
    value: object, cube_id: str, direction: str, slot: int
) -> tuple[SerializedLinkId, ...]:
    """Validate interface-owned link identities from current ComfyUI payloads."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(
            f"Native Cube '{cube_id}' has invalid linkIds on declared "
            f"{direction} slot {slot}"
        )
    link_ids: list[SerializedLinkId] = []
    for link_id in value:
        if isinstance(link_id, bool) or not isinstance(link_id, (int, str)):
            raise ValueError(
                f"Native Cube '{cube_id}' has invalid linkIds on declared "
                f"{direction} slot {slot}"
            )
        if isinstance(link_id, str) and not link_id:
            raise ValueError(
                f"Native Cube '{cube_id}' has invalid linkIds on declared "
                f"{direction} slot {slot}"
            )
        link_ids.append(link_id)
    return tuple(link_ids)


def _io_node_id(value: object, fallback: int) -> SerializedGraphId:
    """Return one native IO-node id while preserving string identities."""

    if not isinstance(value, Mapping):
        return fallback
    node_id = value.get("id", fallback)
    if isinstance(node_id, int) and not isinstance(node_id, bool):
        return node_id
    if isinstance(node_id, str) and node_id:
        return node_id
    return fallback


def _same_id(left: SerializedGraphId, right: SerializedGraphId) -> bool:
    """Compare host ids without conflating numeric and string identities."""

    return type(left) is type(right) and left == right


def _same_link_id(left: object, right: SerializedLinkId) -> bool:
    """Compare dynamic endpoint metadata to one validated link identity."""

    return type(left) is type(right) and left == right


def _invalid_endpoint_error(
    cube_id: str,
    endpoint: str,
    node_id: SerializedGraphId,
    direction: str,
    slot: int,
) -> ValueError:
    """Build the canonical error for invalid node-owned endpoint metadata."""

    return ValueError(
        f"Native Cube '{cube_id}' {endpoint} node '{node_id}' "
        f"{direction} slot {slot} does not own its serialized link"
    )


def _dangling_boundary_error(cube_id: str, direction: str, slot: int) -> ValueError:
    """Build the canonical error for a boundary link without a valid body endpoint."""

    return ValueError(
        f"Native Cube '{cube_id}' declared {direction} slot {slot} "
        "has no valid serialized body-node link"
    )
