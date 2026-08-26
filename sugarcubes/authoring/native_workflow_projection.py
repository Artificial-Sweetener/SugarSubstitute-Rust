#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project native authoring plans into executable embedded-Cube workflows."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy

from .native_workflow_models import NativeWorkflowImportPlan
from ..workflow_node_pack import current_workflow_node_pack


class NativeWorkflowProjectionError(ValueError):
    """Reject an authoring plan that cannot form one canonical workflow."""


def project_native_workflow_plan(
    plan: NativeWorkflowImportPlan,
) -> dict[str, object]:
    """Build a self-contained workflow using each prepared Cube document."""

    nodes: list[dict[str, object]] = []
    node_pack_properties = current_workflow_node_pack().node_properties()
    definitions: list[dict[str, object]] = []
    instances: dict[str, tuple[int, list[str], list[str]]] = {}
    for index, instance in enumerate(plan.instances, start=1):
        document = _mapping(instance.payload.get("document"), "instance document")
        implementation = _mapping(document.get("implementation"), "implementation")
        input_names = _names(implementation.get("inputs"), "Cube inputs")
        output_names = _names(implementation.get("outputs"), "Cube outputs")
        cube_id = _text(document.get("cube_id"), "cube_id")
        version = _text(document.get("version"), "version")
        definition_id = f"sugarcubes-plan-definition-{index}"
        identity = {
            "cube_id": cube_id,
            "cube_version": version,
            "default_alias": instance.alias,
        }
        nodes.append(
            {
                "id": index,
                "type": definition_id,
                "mode": 4 if instance.bypassed else 0,
                "inputs": [{"name": name, "type": "*"} for name in input_names],
                "outputs": [{"name": name, "type": "*"} for name in output_names],
                "properties": {
                    **node_pack_properties,
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": {
                        **identity,
                        "instance_id": instance.instance_id,
                        "instance_alias": instance.alias,
                    },
                },
            }
        )
        definitions.append(
            {
                "id": definition_id,
                "name": instance.alias,
                "nodes": [],
                "links": [],
                "inputs": [{"name": name, "type": "*"} for name in input_names],
                "outputs": [{"name": name, "type": "*"} for name in output_names],
                "extra": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": identity,
                    "sugarcubes_document": deepcopy(document),
                },
            }
        )
        if instance.instance_id in instances:
            raise NativeWorkflowProjectionError(
                f"Duplicate instance id '{instance.instance_id}'"
            )
        instances[instance.instance_id] = (index, input_names, output_names)

    links: list[list[object]] = []
    for link_id, connection in enumerate(plan.connections, start=1):
        source = instances.get(connection.source_instance_id)
        target = instances.get(connection.target_instance_id)
        if source is None or target is None:
            raise NativeWorkflowProjectionError(
                "Connection references an unknown Cube instance"
            )
        source_slot = _unique_slot(source[2], connection.source_binding, "output")
        target_slot = _unique_slot(target[1], connection.target_binding, "input")
        links.append([link_id, source[0], source_slot, target[0], target_slot, "*"])
    return {
        "version": 0.4,
        "nodes": nodes,
        "links": links,
        "definitions": {"subgraphs": definitions},
        "extra": {"sugarcubes_authoring_semantic_hash": plan.semantic_hash},
    }


def _names(value: object, label: str) -> list[str]:
    """Read ordered boundary names from one Cube implementation mapping."""

    if not isinstance(value, Mapping):
        raise NativeWorkflowProjectionError(f"{label} must be an object")
    names = [str(name) for name in value]
    if any(not name.strip() for name in names) or len(set(names)) != len(names):
        raise NativeWorkflowProjectionError(f"{label} contain invalid names")
    return names


def _unique_slot(names: Sequence[str], name: str, direction: str) -> int:
    """Resolve one exact public boundary without suffix or position guessing."""

    matches = [index for index, candidate in enumerate(names) if candidate == name]
    if len(matches) != 1:
        raise NativeWorkflowProjectionError(
            f"Cube exposes no unique {direction} boundary '{name}'"
        )
    return matches[0]


def _mapping(value: object, label: str) -> Mapping[str, object]:
    """Require one string-keyed serialized object."""

    if not isinstance(value, Mapping) or not all(isinstance(key, str) for key in value):
        raise NativeWorkflowProjectionError(f"{label} must be an object")
    return value


def _text(value: object, label: str) -> str:
    """Require one nonempty identity field."""

    if not isinstance(value, str) or not value.strip():
        raise NativeWorkflowProjectionError(f"{label} is required")
    return value.strip()
