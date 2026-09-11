#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Mutate reorderable Cube graph segments through SugarCubes-owned geometry."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
from math import isfinite
from uuid import uuid4

from .cube_model import CubeDocument, CubeSchemaError
from .execution.portable_boundary_lowering import document_boundary_ports
from .workflow_analysis import CubeGraphAnalysis, CubeGraphAnalysisService
from .workflow_node_pack import current_workflow_node_pack

_MINIMUM_SWAP_GAP = 24.0
_DEFAULT_CUBE_WIDTH = 320.0
_DEFAULT_CUBE_HEIGHT = 200.0


class CubeGraphMutationError(ValueError):
    """Reject a graph mutation that would cross or reinterpret a graph boundary."""


@dataclass(frozen=True, slots=True)
class CubeGraphDraft:
    """Describe one exact Cube instance for native graph creation."""

    instance_id: str
    alias: str
    bypassed: bool
    document: object


class CubeGraphMutationService:
    """Own atomic Cube-segment mutations over complete native workflows."""

    def __init__(
        self,
        analysis: CubeGraphAnalysisService,
        id_factory: Callable[[], str] | None = None,
    ) -> None:
        """Bind the canonical analysis owner used before and after mutation."""

        self._analysis = analysis
        self._id_factory = id_factory or (lambda: str(uuid4()))

    def append_cube(
        self,
        workflow_value: object,
        *,
        instance_id: str,
        alias: str,
        bypassed: bool,
        document: object,
    ) -> CubeGraphAnalysis:
        """Append one exact Cube while preserving all unrelated graph content."""

        before = self._analysis.analyze(workflow_value)
        normalized_instance_id = _required_text(instance_id, "instance id")
        normalized_alias = _required_text(alias, "alias")
        if any(
            instance.instance_id == normalized_instance_id
            for instance in before.instances
        ):
            raise CubeGraphMutationError(
                f"Cube instance id '{normalized_instance_id}' is already in use."
            )
        if any(
            instance.instance_alias == normalized_alias for instance in before.instances
        ):
            raise CubeGraphMutationError(
                f"Cube alias '{normalized_alias}' is already in use."
            )
        cube = _cube_document(document)
        mutated = deepcopy(dict(before.normalized_workflow))
        self._append_validated_cube(
            mutated,
            instance_id=normalized_instance_id,
            alias=normalized_alias,
            bypassed=bypassed,
            cube=cube,
        )
        after = self._analysis.analyze(mutated)
        if not any(
            instance.instance_id == normalized_instance_id
            for instance in after.instances
        ):
            raise CubeGraphMutationError(
                "Appended Cube did not survive graph analysis."
            )
        return after

    def create_cube_workflow(
        self,
        drafts: Sequence[CubeGraphDraft],
    ) -> CubeGraphAnalysis:
        """Create one ordered native graph with a single final analysis pass."""

        if not drafts:
            raise CubeGraphMutationError("At least one Cube is required.")
        mutated = _empty_native_workflow()
        instance_ids: set[str] = set()
        aliases: set[str] = set()
        for draft in drafts:
            instance_id = _required_text(draft.instance_id, "instance id")
            alias = _required_text(draft.alias, "alias")
            if instance_id in instance_ids:
                raise CubeGraphMutationError(
                    f"Cube instance id '{instance_id}' is already in use."
                )
            if alias in aliases:
                raise CubeGraphMutationError(f"Cube alias '{alias}' is already in use.")
            instance_ids.add(instance_id)
            aliases.add(alias)
            self._append_validated_cube(
                mutated,
                instance_id=instance_id,
                alias=alias,
                bypassed=draft.bypassed,
                cube=_cube_document(draft.document),
            )
        result = self._analysis.analyze(mutated)
        if {instance.instance_id for instance in result.instances} != instance_ids:
            raise CubeGraphMutationError(
                "Created Cubes did not survive graph analysis."
            )
        return result

    def _append_validated_cube(
        self,
        workflow: dict[str, object],
        *,
        instance_id: str,
        alias: str,
        bypassed: bool,
        cube: CubeDocument,
    ) -> None:
        """Append one validated Cube without triggering an intermediate analysis."""

        nodes = _mutable_node_values(workflow)
        subgraphs = _mutable_subgraphs(workflow)
        node_id, definition_id = self._new_ids(workflow)
        inputs, outputs = document_boundary_ports(cube)
        identity = {
            "cube_id": cube.cube_id,
            "cube_version": cube.version,
            "default_alias": alias,
        }
        left, top = _next_cube_position(nodes)
        nodes.append(
            {
                "id": node_id,
                "type": definition_id,
                "mode": 4 if bypassed else 0,
                "pos": [left, top],
                "size": [_DEFAULT_CUBE_WIDTH, _DEFAULT_CUBE_HEIGHT],
                "inputs": deepcopy(list(inputs)),
                "outputs": deepcopy(list(outputs)),
                "properties": {
                    **current_workflow_node_pack().node_properties(),
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": {
                        **identity,
                        "instance_id": instance_id,
                        "instance_alias": alias,
                    },
                },
            }
        )
        subgraphs.append(
            {
                "id": definition_id,
                "name": alias,
                "nodes": [],
                "links": [],
                "inputs": deepcopy(list(inputs)),
                "outputs": deepcopy(list(outputs)),
                "extra": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": identity,
                    "sugarcubes_document": cube.to_dict(),
                },
            }
        )

    def remove_cube(
        self,
        workflow_value: object,
        *,
        instance_id: str,
    ) -> CubeGraphAnalysis:
        """Remove one recognized Cube and only the graph records it owns."""

        before = self._analysis.analyze(workflow_value)
        normalized_instance_id = _required_text(instance_id, "instance id")
        instance = next(
            (
                candidate
                for candidate in before.instances
                if candidate.instance_id == normalized_instance_id
            ),
            None,
        )
        if instance is None:
            raise CubeGraphMutationError("Cube instance is unavailable.")
        mutated = deepcopy(dict(before.normalized_workflow))
        nodes = _mutable_node_values(mutated)
        remaining_nodes = [
            node for node in nodes if str(node.get("id")) != instance.node_id
        ]
        if len(remaining_nodes) == len(nodes):
            raise CubeGraphMutationError("Cube graph node is unavailable.")
        mutated["nodes"] = remaining_nodes
        links = mutated.get("links")
        if isinstance(links, list):
            mutated["links"] = [
                link for link in links if not _link_touches(link, instance.node_id)
            ]
        if not any(
            str(node.get("type")) == instance.definition_id for node in remaining_nodes
        ):
            subgraphs = _mutable_subgraphs(mutated)
            _replace_subgraphs(
                mutated,
                [
                    definition
                    for definition in subgraphs
                    if str(definition.get("id")) != instance.definition_id
                ],
            )
        after = self._analysis.analyze(mutated)
        if any(
            candidate.instance_id == normalized_instance_id
            for candidate in after.instances
        ):
            raise CubeGraphMutationError("Removed Cube remains in graph analysis.")
        return after

    def _new_ids(self, workflow: Mapping[str, object]) -> tuple[str, str]:
        """Allocate collision-resistant root-node and definition identifiers."""

        occupied = _occupied_ids(workflow)
        allocated: list[str] = []
        for _attempt in range(32):
            candidate = self._id_factory().strip()
            if candidate and candidate not in occupied and candidate not in allocated:
                allocated.append(candidate)
                if len(allocated) == 2:
                    return allocated[0], allocated[1]
        raise CubeGraphMutationError(
            "Unable to allocate unique Cube graph identifiers."
        )

    def reorder(
        self,
        workflow_value: object,
        *,
        segment_instance_ids: Sequence[str],
        ordered_instance_ids: Sequence[str],
    ) -> CubeGraphAnalysis:
        """Reorder one proximity-only segment using native Cube swap geometry."""

        before = self._analysis.analyze(workflow_value)
        segment_identity = tuple(segment_instance_ids)
        requested = tuple(ordered_instance_ids)
        segment = next(
            (
                candidate
                for candidate in before.segments
                if candidate.instance_ids == segment_identity
            ),
            None,
        )
        if segment is None:
            raise CubeGraphMutationError("Cube graph segment is unavailable.")
        if not segment.reorderable:
            raise CubeGraphMutationError(
                "Cube graph segment is fixed by explicit wiring or an ordinary graph boundary."
            )
        if len(requested) != len(set(requested)) or set(requested) != set(
            segment.instance_ids
        ):
            raise CubeGraphMutationError(
                "Cube reorder must be an exact segment permutation."
            )
        mutated = deepcopy(dict(before.normalized_workflow))
        nodes = _mutable_nodes(mutated)
        node_ids = {
            instance.instance_id: instance.node_id for instance in before.instances
        }
        current = list(segment.instance_ids)
        for requested_index, instance_id in enumerate(requested):
            current_index = current.index(instance_id)
            while current_index > requested_index:
                left_id = current[current_index - 1]
                right_id = current[current_index]
                _swap_nodes(nodes[node_ids[left_id]], nodes[node_ids[right_id]])
                current[current_index - 1], current[current_index] = (
                    current[current_index],
                    current[current_index - 1],
                )
                current_index -= 1
        after = self._analysis.analyze(mutated)
        if not any(candidate.instance_ids == requested for candidate in after.segments):
            raise CubeGraphMutationError(
                "Reordered Cube geometry did not produce the requested topology."
            )
        return after


def _mutable_nodes(workflow: dict[str, object]) -> dict[str, dict[str, object]]:
    """Index mutable root nodes by normalized Comfy identifier."""

    values = workflow.get("nodes")
    if not isinstance(values, list):
        raise CubeGraphMutationError("Workflow nodes are unavailable.")
    result: dict[str, dict[str, object]] = {}
    for value in values:
        if not isinstance(value, dict):
            continue
        node_id = value.get("id")
        if isinstance(node_id, bool) or not isinstance(node_id, str | int):
            continue
        result[str(node_id)] = value
    return result


def _mutable_node_values(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the mutable root-node array without filtering ordinary records."""

    values = workflow.get("nodes")
    if not isinstance(values, list) or not all(
        isinstance(value, dict) for value in values
    ):
        raise CubeGraphMutationError("Workflow nodes are unavailable.")
    return values


def _mutable_subgraphs(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the mutable native-definition array without interpreting non-Cubes."""

    definitions = workflow.setdefault("definitions", {})
    if not isinstance(definitions, dict):
        raise CubeGraphMutationError("Workflow definitions are unavailable.")
    values = definitions.setdefault("subgraphs", [])
    if not isinstance(values, list) or not all(
        isinstance(value, dict) for value in values
    ):
        raise CubeGraphMutationError("Workflow subgraph definitions are unavailable.")
    return values


def _replace_subgraphs(
    workflow: dict[str, object],
    subgraphs: list[dict[str, object]],
) -> None:
    """Replace only the definition array in one validated mutable workflow."""

    definitions = workflow.get("definitions")
    if not isinstance(definitions, dict):
        raise CubeGraphMutationError("Workflow definitions are unavailable.")
    definitions["subgraphs"] = subgraphs


def _next_cube_position(nodes: Sequence[Mapping[str, object]]) -> tuple[float, float]:
    """Place after the rightmost marked Cube without consulting ordinary geometry."""

    geometries = [
        geometry
        for node in nodes
        if _is_cube_node(node) and (geometry := _optional_geometry(node)) is not None
    ]
    if not geometries:
        return 0.0, 0.0
    left, top, width = max(geometries, key=lambda value: value[0] + value[2])
    return left + width + _MINIMUM_SWAP_GAP, top


def _is_cube_node(node: Mapping[str, object]) -> bool:
    """Recognize only explicit SugarCubes root markers."""

    properties = node.get("properties")
    return isinstance(properties, Mapping) and properties.get("sugarcubes_kind") in {
        "cube",
        "cube_draft",
    }


def _optional_geometry(
    node: Mapping[str, object],
) -> tuple[float, float, float] | None:
    """Read valid Cube placement without rejecting unrelated malformed geometry."""

    try:
        left, top = _pair(node.get("pos"), "position")
        width, _height = _pair(node.get("size"), "size")
    except CubeGraphMutationError:
        return None
    return left, top, max(0.0, width)


def _occupied_ids(workflow: Mapping[str, object]) -> set[str]:
    """Collect root and definition identifiers without interpreting payloads."""

    nodes = workflow.get("nodes")
    definitions = workflow.get("definitions")
    subgraphs = (
        definitions.get("subgraphs") if isinstance(definitions, Mapping) else None
    )
    values = [
        *(nodes if isinstance(nodes, list) else []),
        *(subgraphs if isinstance(subgraphs, list) else []),
    ]
    return {
        str(value["id"])
        for value in values
        if isinstance(value, Mapping) and "id" in value
    }


def _link_touches(link: object, node_id: str) -> bool:
    """Return whether one supported persisted link touches an exact node id."""

    if isinstance(link, Mapping):
        return node_id in {str(link.get("origin_id")), str(link.get("target_id"))}
    if (
        isinstance(link, Sequence)
        and not isinstance(link, (str, bytes, bytearray))
        and len(link) >= 4
    ):
        return node_id in {str(link[1]), str(link[3])}
    return False


def _required_text(value: str, label: str) -> str:
    """Normalize one required mutation identity."""

    normalized = value.strip()
    if not normalized:
        raise CubeGraphMutationError(f"Cube {label} is required.")
    return normalized


def _cube_document(value: object) -> CubeDocument:
    """Validate one exact Cube document before mutating graph state."""

    if not isinstance(value, Mapping):
        raise CubeGraphMutationError("Cube document is invalid: expected an object.")
    try:
        return CubeDocument.from_dict(value)
    except (CubeSchemaError, TypeError, ValueError) as error:
        raise CubeGraphMutationError(f"Cube document is invalid: {error}") from error


def _empty_native_workflow() -> dict[str, object]:
    """Return the canonical empty Comfy graph envelope for stack migration."""

    return {
        "version": 0.4,
        "nodes": [],
        "links": [],
        "definitions": {"subgraphs": []},
        "extra": {},
    }


def _swap_nodes(left: dict[str, object], right: dict[str, object]) -> None:
    """Apply the same collision-safe adjacent placement used by the frontend."""

    left_x, left_y = _pair(left.get("pos"), "position")
    right_x, right_y = _pair(right.get("pos"), "position")
    left_width, _left_height = _pair(left.get("size"), "size")
    right_width, _right_height = _pair(right.get("size"), "size")
    authored_gap = right_x - (left_x + max(0.0, left_width))
    gap = max(_MINIMUM_SWAP_GAP, authored_gap, 0.0)
    left["pos"] = [left_x + max(0.0, right_width) + gap, right_y]
    right["pos"] = [left_x, left_y]


def _pair(value: object, label: str) -> tuple[float, float]:
    """Read one finite serialized geometry pair without coercing arbitrary values."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise CubeGraphMutationError(f"Cube {label} is unavailable.")
    if len(value) < 2:
        raise CubeGraphMutationError(f"Cube {label} is incomplete.")
    first, second = value[0], value[1]
    if (
        isinstance(first, bool)
        or not isinstance(first, int | float)
        or isinstance(second, bool)
        or not isinstance(second, int | float)
        or not isfinite(float(first))
        or not isfinite(float(second))
    ):
        raise CubeGraphMutationError(f"Cube {label} must contain finite numbers.")
    return float(first), float(second)


__all__ = ["CubeGraphDraft", "CubeGraphMutationError", "CubeGraphMutationService"]
