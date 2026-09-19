#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Replace canonical Cube definitions without replacing native instances."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy

from .cube_model import CubeDocument, CubeSchemaError
from .execution.portable_boundary_lowering import document_boundary_ports
from .workflow_analysis import CubeGraphAnalysis, CubeGraphAnalysisService
from .workflow_mutation_errors import CubeGraphMutationError


class CubeGraphReplacementService:
    """Own identity-preserving canonical Cube definition replacement."""

    def __init__(self, analysis: CubeGraphAnalysisService) -> None:
        """Bind the canonical analysis owner used before and after replacement."""

        self._analysis = analysis

    def replace_cube(
        self,
        workflow_value: object,
        *,
        instance_id: str,
        document: object,
    ) -> CubeGraphAnalysis:
        """Replace one Cube document while preserving its native graph identity."""

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
        cube = _cube_document(document)
        if cube.cube_id != instance.cube_id:
            raise CubeGraphMutationError(
                "Replacement Cube identity must match the existing instance."
            )
        mutated = deepcopy(dict(before.normalized_workflow))
        node = _mutable_nodes(mutated).get(instance.node_id)
        definition = next(
            (
                candidate
                for candidate in _mutable_subgraphs(mutated)
                if str(candidate.get("id")) == instance.definition_id
            ),
            None,
        )
        if node is None or definition is None:
            raise CubeGraphMutationError("Cube graph identity is incomplete.")
        inputs, outputs = document_boundary_ports(cube)
        connected = any(
            _link_touches(link, instance.node_id) for link in _workflow_links(mutated)
        )
        _replace_boundary_ports(node, "inputs", inputs, require_same_order=connected)
        _replace_boundary_ports(node, "outputs", outputs, require_same_order=connected)
        definition["inputs"] = deepcopy(list(inputs))
        definition["outputs"] = deepcopy(list(outputs))
        default_alias = str(
            cube.metadata.get("default_alias") or instance.instance_alias
        )
        identity = {
            "cube_id": cube.cube_id,
            "cube_version": cube.version,
            "default_alias": default_alias,
        }
        properties = node.get("properties")
        if not isinstance(properties, dict):
            raise CubeGraphMutationError("Cube graph node properties are unavailable.")
        marker = properties.get("sugarcubes_cube")
        if not isinstance(marker, dict):
            raise CubeGraphMutationError(
                "Cube instance identity marker is unavailable."
            )
        properties["sugarcubes_cube"] = {
            **marker,
            **identity,
            "instance_id": instance.instance_id,
            "instance_alias": instance.instance_alias,
        }
        extra = definition.get("extra")
        if not isinstance(extra, dict):
            raise CubeGraphMutationError("Cube definition metadata is unavailable.")
        definition_marker = extra.get("sugarcubes_cube")
        extra["sugarcubes_cube"] = {
            **(definition_marker if isinstance(definition_marker, dict) else {}),
            **identity,
        }
        extra["sugarcubes_document"] = cube.to_dict()
        definition["name"] = instance.instance_alias
        after = self._analysis.analyze(mutated)
        replaced = next(
            (
                candidate
                for candidate in after.instances
                if candidate.instance_id == normalized_instance_id
            ),
            None,
        )
        if (
            replaced is None
            or replaced.node_id != instance.node_id
            or replaced.definition_id != instance.definition_id
            or replaced.instance_alias != instance.instance_alias
            or replaced.cube_version != cube.version
        ):
            raise CubeGraphMutationError(
                "Replacement Cube did not preserve native graph identity."
            )
        return after


def _mutable_nodes(workflow: dict[str, object]) -> dict[str, dict[str, object]]:
    """Index mutable root nodes by normalized Comfy identifier."""

    values = workflow.get("nodes")
    if not isinstance(values, list):
        raise CubeGraphMutationError("Workflow nodes are unavailable.")
    return {
        str(value["id"]): value
        for value in values
        if isinstance(value, dict)
        and "id" in value
        and not isinstance(value["id"], bool)
        and isinstance(value["id"], str | int)
    }


def _mutable_subgraphs(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the mutable native-definition array."""

    definitions = workflow.get("definitions")
    values = definitions.get("subgraphs") if isinstance(definitions, dict) else None
    if not isinstance(values, list) or not all(
        isinstance(value, dict) for value in values
    ):
        raise CubeGraphMutationError("Workflow subgraph definitions are unavailable.")
    return values


def _workflow_links(workflow: Mapping[str, object]) -> list[object]:
    """Return persisted workflow links without interpreting their payloads."""

    links = workflow.get("links")
    return links if isinstance(links, list) else []


def _replace_boundary_ports(
    owner: dict[str, object],
    key: str,
    values: Sequence[Mapping[str, object]],
    *,
    require_same_order: bool,
) -> None:
    """Replace boundary declarations while retaining host connection metadata."""

    existing = owner.get(key)
    existing_ports = (
        [value for value in existing if isinstance(value, Mapping)]
        if isinstance(existing, list)
        else []
    )
    existing_names = tuple(str(value.get("name", "")) for value in existing_ports)
    next_names = tuple(str(value.get("name", "")) for value in values)
    if require_same_order and existing_names != next_names:
        raise CubeGraphMutationError(
            "Connected Cube boundary ports cannot change during replacement."
        )
    existing_by_name = {str(value.get("name", "")): value for value in existing_ports}
    replaced: list[dict[str, object]] = []
    for value in values:
        port = deepcopy(dict(value))
        previous = existing_by_name.get(str(value.get("name", "")))
        if previous is not None:
            for connection_key in ("link", "links"):
                if connection_key in previous:
                    port[connection_key] = deepcopy(previous[connection_key])
        replaced.append(port)
    owner[key] = replaced


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
    """Normalize one required replacement identity."""

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


__all__ = ["CubeGraphReplacementService"]
