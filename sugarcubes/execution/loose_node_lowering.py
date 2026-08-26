#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project ordinary root workflow nodes as explicit loose execution owners."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping, MutableMapping, Sequence

from ..cube_model.widget_values import (
    WidgetSnapshotError,
    decode_workflow_widget_snapshot,
)
from ..workflow import CanonicalWorkflow
from .errors import CubeLoweringError
from .lowering_values import identifier, node_mode
from .models import NodeOwner

_DISABLED_MODE = 2
_BYPASS_MODE = 4


class LooseWorkflowNodeLowerer:
    """Own root loose-node projection and its strict ownership barrier."""

    def lower(
        self,
        workflow: CanonicalWorkflow,
        prompt: MutableMapping[str, dict[str, object]],
        owners: MutableMapping[str, NodeOwner],
        definitions: MutableMapping[str, Mapping[str, object]],
    ) -> None:
        """Append executable root nodes without claiming Cube semantics for them."""

        cube_node_ids = {instance.node_id for instance in workflow.instances}
        nodes = workflow.payload.get("nodes")
        if not isinstance(nodes, Sequence) or isinstance(
            nodes, (str, bytes, bytearray)
        ):
            return
        root_links = root_link_sources(workflow.payload)
        for raw_node in nodes:
            if not isinstance(raw_node, Mapping):
                continue
            node_id = identifier(raw_node.get("id"))
            if node_id is None or node_id in cube_node_ids:
                continue
            if node_mode(raw_node) in {_DISABLED_MODE, _BYPASS_MODE}:
                continue
            class_type = raw_node.get("type")
            if not isinstance(class_type, str) or not class_type.strip():
                raise CubeLoweringError(
                    "execution.lowering.invalid_loose_node",
                    f"Loose workflow node '{node_id}' has no valid type.",
                    path=f"$.nodes[{node_id}].type",
                )
            prompt[node_id] = {
                "class_type": class_type.strip(),
                "inputs": _loose_inputs(raw_node, root_links),
            }
            owners[node_id] = NodeOwner(None)
            definitions[node_id] = {}


def root_link_sources(payload: Mapping[str, object]) -> dict[object, tuple[str, int]]:
    """Index root link ids for loose-node input projection."""

    links = payload.get("links")
    if not isinstance(links, Sequence) or isinstance(links, (str, bytes, bytearray)):
        return {}
    result: dict[object, tuple[str, int]] = {}
    for link in links:
        if isinstance(link, Mapping):
            link_id = link.get("id")
            source = identifier(link.get("origin_id"))
            slot = link.get("origin_slot")
        elif (
            isinstance(link, Sequence)
            and not isinstance(link, (str, bytes, bytearray))
            and len(link) >= 3
        ):
            link_id, source, slot = link[0], identifier(link[1]), link[2]
        else:
            continue
        if source is not None and isinstance(slot, int) and not isinstance(slot, bool):
            result[link_id] = (source, slot)
    return result


def _loose_inputs(
    node: Mapping[str, object],
    links: Mapping[object, tuple[str, int]],
) -> dict[str, object]:
    """Project named root inputs and stable widget values for a loose node."""

    entries = node.get("inputs")
    result: dict[str, object] = {}
    if not isinstance(entries, Sequence) or isinstance(
        entries, (str, bytes, bytearray)
    ):
        entries = []
    linked_names: set[str] = set()
    for entry in entries:
        if not isinstance(entry, Mapping):
            continue
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            continue
        link = entry.get("link")
        if link in links:
            source, slot = links[link]
            result[name] = [source, slot]
            linked_names.add(name)
            continue
        result[name] = None
    try:
        snapshot = decode_workflow_widget_snapshot(node, {})
    except WidgetSnapshotError as error:
        node_id = identifier(node.get("id")) or "?"
        class_type = node.get("type", node.get("class_type"))
        raise CubeLoweringError(
            "execution.lowering.ambiguous_loose_widget_values",
            f"Loose workflow node '{node_id}' ({class_type}) has ambiguous widget "
            f"layout: {error}",
            path=f"$.nodes[{node_id}].widgets_values",
        ) from error
    if snapshot is not None:
        for name, value in snapshot.values.items():
            if name not in linked_names:
                result[name] = deepcopy(value)
    return result
