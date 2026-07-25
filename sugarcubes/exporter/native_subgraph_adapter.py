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
"""Project native Cube subgraphs into the stable marker-era export contract."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from typing import Any, Mapping, MutableMapping, Sequence

from .graph import MARKER_CLASS_TYPES


@dataclass(frozen=True)
class _Link:
    """Describe one validated serialized Comfy subgraph link."""

    origin_id: object
    origin_slot: int
    target_id: object
    target_slot: int
    link_type: object


@dataclass
class _ProjectionState:
    """Own mutable payloads and synthetic link allocation for one projection."""

    prompt: MutableMapping[str, Any]
    workflow_nodes: list[Any]
    workflow_links: list[Any]
    next_link_id: int

    def allocate_link_id(self) -> int:
        """Return one workflow-unique synthetic link id."""

        value = self.next_link_id
        self.next_link_id += 1
        return value


def project_native_cube_exports(
    graph: Mapping[str, Any],
    workflow: Mapping[str, Any],
    cube_entries: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Return export-only payloads with native Cube boundaries represented explicitly.

    The projection exists only at the server adapter boundary. It does not create
    marker nodes, child projections, or groups in the live Comfy graph.
    """

    native_entries = {
        cube_id: entry
        for cube_id, entry in cube_entries.items()
        if _nonempty_string(entry.get("definition_id"))
        and (
            _string_list(entry.get("instance_container_ids"))
            or _string_list(entry.get("instance_node_ids"))
        )
    }
    if not native_entries:
        return deepcopy(dict(graph)), deepcopy(dict(workflow))

    projected_graph = deepcopy(dict(graph))
    prompt = _mutable_prompt(projected_graph)
    projected_workflow = deepcopy(dict(workflow))
    workflow_nodes = _mutable_list(projected_workflow, "nodes")
    workflow_links = _mutable_list(projected_workflow, "links")
    definitions = _subgraph_index(projected_workflow)
    root_nodes = {
        str(node.get("id")): node
        for node in workflow_nodes
        if isinstance(node, Mapping) and node.get("id") is not None
    }
    containers = _container_index(projected_workflow)
    state = _ProjectionState(
        prompt=prompt,
        workflow_nodes=workflow_nodes,
        workflow_links=workflow_links,
        next_link_id=_next_workflow_link_id(workflow_links),
    )

    for cube_index, (cube_id, entry) in enumerate(native_entries.items()):
        definition_id = _nonempty_string(entry.get("definition_id"))
        definition = definitions.get(definition_id)
        if definition is None:
            raise ValueError(
                f"Native Cube '{cube_id}' references missing subgraph '{definition_id}'"
            )
        container_ids = _string_list(entry.get("instance_container_ids"))
        instance_id = next(
            (
                candidate
                for candidate in container_ids
                if candidate in containers
                and _nonempty_string(containers[candidate].get("definition_id"))
                == definition_id
            ),
            "",
        )
        if not instance_id:
            node_ids = _string_list(entry.get("instance_node_ids"))
            instance_id = next(
                (
                    candidate
                    for candidate in node_ids
                    if candidate in root_nodes
                    and _nonempty_string(root_nodes[candidate].get("type"))
                    == definition_id
                ),
                "",
            )
        if not instance_id:
            raise ValueError(
                f"Native Cube '{cube_id}' has no matching surface instance for '{definition_id}'"
            )
        _project_definition(
            state,
            cube_id=cube_id,
            default_alias=_entry_default_alias(cube_id, entry),
            cube_index=cube_index,
            instance_id=instance_id,
            definition=definition,
        )

    return projected_graph, projected_workflow


def _container_index(workflow: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    """Index the extension-owned non-node container persistence envelope."""

    extra = workflow.get("extra")
    if not isinstance(extra, Mapping):
        return {}
    envelope = extra.get("sugarcubes_containers")
    if not isinstance(envelope, Mapping):
        return {}
    items = envelope.get("items")
    if not isinstance(items, Sequence) or isinstance(items, (str, bytes)):
        return {}
    return {
        container_id: item
        for item in items
        if isinstance(item, Mapping)
        and (container_id := _nonempty_string(item.get("id")))
    }


def _project_definition(
    state: _ProjectionState,
    *,
    cube_id: str,
    default_alias: str,
    cube_index: int,
    instance_id: str,
    definition: Mapping[str, Any],
) -> None:
    """Project one immediate Cube definition and its graph-owned boundaries."""

    definition_nodes = definition.get("nodes")
    if not isinstance(definition_nodes, Sequence) or isinstance(
        definition_nodes, (str, bytes)
    ):
        raise ValueError(f"Native Cube '{cube_id}' has no serialized subgraph nodes")

    projected_ids: dict[str, str] = {}
    for raw_node in definition_nodes:
        if not isinstance(raw_node, Mapping) or raw_node.get("id") is None:
            continue
        internal_id = str(raw_node["id"])
        projected_id = f"{instance_id}:{internal_id}"
        projected_ids[internal_id] = projected_id
        projected_node = deepcopy(dict(raw_node))
        projected_node["id"] = projected_id
        _clear_definition_local_links(projected_node)
        state.workflow_nodes.append(projected_node)

    input_node_id = str(_io_node_id(definition.get("inputNode"), -10))
    output_node_id = str(_io_node_id(definition.get("outputNode"), -20))
    input_markers: dict[int, str] = {}
    output_markers: dict[int, str] = {}
    input_names = _boundary_names(definition.get("inputs"))
    output_names = _boundary_names(definition.get("outputs"))

    raw_links = definition.get("links")
    if not isinstance(raw_links, Sequence) or isinstance(raw_links, (str, bytes)):
        raw_links = []
    for raw_link in raw_links:
        link = _read_link(raw_link)
        if link is None:
            continue
        origin_key = str(link.origin_id)
        target_key = str(link.target_id)
        projected_origin = projected_ids.get(origin_key)
        projected_target = projected_ids.get(target_key)
        if origin_key == input_node_id and projected_target:
            marker_id = input_markers.get(link.origin_slot)
            if marker_id is None:
                marker_id = _marker_id(cube_index, "input", link.origin_slot)
                input_markers[link.origin_slot] = marker_id
                _add_marker(
                    state,
                    marker_id=marker_id,
                    marker_type="SugarCubes.CubeInput",
                    cube_id=cube_id,
                    default_alias=default_alias,
                    boundary_name=input_names.get(
                        link.origin_slot, f"input.{link.origin_slot}"
                    ),
                    position=_boundary_position(
                        definition.get("inputNode"), link.origin_slot
                    ),
                )
            _add_projected_link(
                state,
                origin_id=marker_id,
                origin_slot=0,
                target_id=projected_target,
                target_slot=link.target_slot,
                link_type=link.link_type,
            )
            continue
        if target_key == output_node_id and projected_origin:
            marker_id = output_markers.get(link.target_slot)
            if marker_id is None:
                marker_id = _marker_id(cube_index, "output", link.target_slot)
                output_markers[link.target_slot] = marker_id
                _add_marker(
                    state,
                    marker_id=marker_id,
                    marker_type="SugarCubes.CubeOutput",
                    cube_id=cube_id,
                    default_alias=default_alias,
                    boundary_name=output_names.get(
                        link.target_slot, f"output.{link.target_slot}"
                    ),
                    position=_boundary_position(
                        definition.get("outputNode"), link.target_slot
                    ),
                )
            _add_projected_link(
                state,
                origin_id=projected_origin,
                origin_slot=link.origin_slot,
                target_id=marker_id,
                target_slot=0,
                link_type=link.link_type,
            )
            continue
        if projected_origin and projected_target:
            _add_projected_link(
                state,
                origin_id=projected_origin,
                origin_slot=link.origin_slot,
                target_id=projected_target,
                target_slot=link.target_slot,
                link_type=link.link_type,
            )


def _add_marker(
    state: _ProjectionState,
    *,
    marker_id: str,
    marker_type: str,
    cube_id: str,
    default_alias: str,
    boundary_name: str,
    position: list[float],
) -> None:
    """Add one export-only boundary marker to prompt and workflow payloads."""

    if marker_type not in MARKER_CLASS_TYPES:
        raise ValueError(f"Unsupported native Cube boundary marker '{marker_type}'")
    prompt_inputs: dict[str, Any] = {
        "cube_id": cube_id,
        "default_alias": default_alias,
        "instance_alias": default_alias,
        "instance_id": "",
    }
    state.prompt[marker_id] = {
        "class_type": marker_type,
        "inputs": prompt_inputs,
        "_meta": {"title": boundary_name},
    }
    state.workflow_nodes.append(
        {
            "id": marker_id,
            "type": marker_type,
            "title": boundary_name,
            "pos": position,
            "size": [140, 46],
            "inputs": [{"name": "value", "type": "*", "link": None}],
            "outputs": [{"name": "value", "type": "*", "links": []}],
        }
    )


def _add_projected_link(
    state: _ProjectionState,
    *,
    origin_id: str,
    origin_slot: int,
    target_id: str,
    target_slot: int,
    link_type: object,
) -> None:
    """Add one export-only link and synchronize both serialized endpoints."""

    link_id = state.allocate_link_id()
    state.workflow_links.append(
        [link_id, origin_id, origin_slot, target_id, target_slot, link_type]
    )
    target_workflow = _find_mutable_workflow_node(state.workflow_nodes, target_id)
    _set_workflow_input_link(target_workflow, target_slot, link_id)
    origin_workflow = _find_mutable_workflow_node(state.workflow_nodes, origin_id)
    _append_workflow_output_link(origin_workflow, origin_slot, link_id)
    target_prompt = state.prompt.get(target_id)
    if isinstance(target_prompt, MutableMapping):
        target_inputs = target_prompt.get("inputs")
        if not isinstance(target_inputs, MutableMapping):
            target_inputs = {}
            target_prompt["inputs"] = target_inputs
        target_name = _input_name(target_workflow, target_slot)
        if target_name:
            target_inputs[target_name] = [origin_id, origin_slot]


def _read_link(value: object) -> _Link | None:
    """Validate one object- or tuple-shaped Comfy serialized link."""

    if isinstance(value, Mapping):
        origin_id = value.get("origin_id")
        origin_slot = value.get("origin_slot")
        target_id = value.get("target_id")
        target_slot = value.get("target_slot")
        link_type = value.get("type", "*")
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        if len(value) < 5:
            return None
        origin_id = value[1]
        origin_slot = value[2]
        target_id = value[3]
        target_slot = value[4]
        link_type = value[5] if len(value) > 5 else "*"
    else:
        return None
    if (
        origin_id is None
        or target_id is None
        or isinstance(origin_slot, bool)
        or not isinstance(origin_slot, int)
        or isinstance(target_slot, bool)
        or not isinstance(target_slot, int)
    ):
        return None
    return _Link(origin_id, origin_slot, target_id, target_slot, link_type)


def _mutable_prompt(graph: MutableMapping[str, Any]) -> MutableMapping[str, Any]:
    """Return the mutable prompt mapping from wrapped or direct graph data."""

    prompt = graph.get("prompt")
    if isinstance(prompt, MutableMapping):
        return prompt
    return graph


def _mutable_list(payload: MutableMapping[str, Any], key: str) -> list[Any]:
    """Return a copied mutable list at one workflow field."""

    value = payload.get(key)
    result = list(value) if isinstance(value, list) else []
    payload[key] = result
    return result


def _subgraph_index(workflow: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    """Index serialized native subgraph definitions by UUID."""

    definitions = workflow.get("definitions")
    if not isinstance(definitions, Mapping):
        return {}
    subgraphs = definitions.get("subgraphs")
    if not isinstance(subgraphs, Sequence) or isinstance(subgraphs, (str, bytes)):
        return {}
    return {
        str(entry["id"]): entry
        for entry in subgraphs
        if isinstance(entry, Mapping)
        and isinstance(entry.get("id"), str)
        and entry["id"]
    }


def _boundary_names(value: object) -> dict[int, str]:
    """Return graph interface names keyed by serialized slot order."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return {}
    result: dict[int, str] = {}
    for index, entry in enumerate(value):
        if not isinstance(entry, Mapping):
            continue
        name = _nonempty_string(entry.get("name")) or _nonempty_string(
            entry.get("label")
        )
        if name:
            result[index] = name
    return result


def _boundary_position(value: object, slot: int) -> list[float]:
    """Derive stable compatibility marker geometry from native IO-node bounds."""

    if isinstance(value, Mapping):
        bounding = value.get("bounding")
        if (
            isinstance(bounding, Sequence)
            and not isinstance(bounding, (str, bytes))
            and len(bounding) >= 2
        ):
            x = _finite_float(bounding[0], 0.0)
            y = _finite_float(bounding[1], 0.0)
            return [x, y + slot * 54.0]
    return [0.0, slot * 54.0]


def _io_node_id(value: object, fallback: int) -> object:
    """Return a serialized graph IO node id."""

    return value.get("id", fallback) if isinstance(value, Mapping) else fallback


def _marker_id(cube_index: int, kind: str, slot: int) -> str:
    """Build one collision-resistant export-only marker id."""

    return f"sugarcubes:native:{cube_index}:{kind}:{slot}"


def _entry_default_alias(cube_id: str, entry: Mapping[str, Any]) -> str:
    """Resolve the native entry's authored display alias."""

    metadata = entry.get("metadata")
    if isinstance(metadata, Mapping):
        alias = _nonempty_string(metadata.get("default_alias"))
        if alias:
            return alias
    return cube_id


def _next_workflow_link_id(links: Sequence[object]) -> int:
    """Return the first positive link id unused by the workflow."""

    maximum = 0
    for value in links:
        if isinstance(value, Mapping):
            link_id = value.get("id")
        elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
            link_id = value[0] if value else None
        else:
            link_id = None
        if isinstance(link_id, int) and not isinstance(link_id, bool):
            maximum = max(maximum, link_id)
    return maximum + 1


def _find_mutable_workflow_node(
    nodes: Sequence[object], node_id: str
) -> MutableMapping[str, Any]:
    """Return one projected workflow node by id."""

    return next(
        (
            node
            for node in nodes
            if isinstance(node, MutableMapping) and str(node.get("id")) == node_id
        ),
        {},
    )


def _set_workflow_input_link(
    node: MutableMapping[str, Any], slot: int, link_id: int
) -> None:
    """Set one target input to the synthetic root-workflow link id."""

    inputs = node.get("inputs")
    if not isinstance(inputs, list) or slot < 0 or slot >= len(inputs):
        return
    entry = inputs[slot]
    if isinstance(entry, MutableMapping):
        entry["link"] = link_id


def _clear_definition_local_links(node: MutableMapping[str, Any]) -> None:
    """Remove definition-scoped link ids before root-workflow projection."""

    inputs = node.get("inputs")
    if isinstance(inputs, list):
        for entry in inputs:
            if isinstance(entry, MutableMapping):
                entry["link"] = None
    outputs = node.get("outputs")
    if isinstance(outputs, list):
        for entry in outputs:
            if isinstance(entry, MutableMapping):
                entry["links"] = []


def _append_workflow_output_link(
    node: MutableMapping[str, Any], slot: int, link_id: int
) -> None:
    """Append one synthetic root-workflow link id to its output slot."""

    outputs = node.get("outputs")
    if not isinstance(outputs, list) or slot < 0 or slot >= len(outputs):
        return
    entry = outputs[slot]
    if not isinstance(entry, MutableMapping):
        return
    links = entry.get("links")
    if not isinstance(links, list):
        links = []
        entry["links"] = links
    links.append(link_id)


def _input_name(node: Mapping[str, Any], slot: int) -> str:
    """Resolve one serialized node input name by slot."""

    inputs = node.get("inputs")
    if not isinstance(inputs, Sequence) or isinstance(inputs, (str, bytes)):
        return ""
    if slot < 0 or slot >= len(inputs):
        return ""
    entry = inputs[slot]
    return _nonempty_string(entry.get("name")) if isinstance(entry, Mapping) else ""


def _string_list(value: object) -> list[str]:
    """Normalize a sequence of non-empty host ids."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [
        normalized
        for entry in value
        if (normalized := _nonempty_string(entry))
    ]


def _nonempty_string(value: object) -> str:
    """Return one trimmed string or an empty value."""

    return value.strip() if isinstance(value, str) else ""


def _finite_float(value: object, fallback: float) -> float:
    """Return one finite numeric geometry value."""

    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return fallback
    numeric = float(value)
    return numeric if numeric == numeric and abs(numeric) != float("inf") else fallback
