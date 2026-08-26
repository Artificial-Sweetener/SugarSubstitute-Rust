#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Reconcile group-era Cube workflows through exact versioned Cube artifacts."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
from typing import Any

from ..cube_model import CubeDocument
from ..cube_model.merge import materialize_nodes
from ..language.compiler_models import (
    CompiledCubeConnection,
    SugarScriptCubeResolver,
)
from .native_workflow_models import NativeCubeImport, NativeWorkflowImportPlan
from .native_workflow_service import NativeCubeImportPreparer
from .legacy_workflow_values import (
    LegacyWorkflowImportError,
    reconcile_saved_node_values,
    saved_symbol,
    surface_widget_names,
)


@dataclass(frozen=True)
class _LegacyInstance:
    """Hold one validated managed group and its owned saved node records."""

    instance_id: str
    alias: str
    cube_id: str
    version: str
    metadata: Mapping[str, object]
    nodes: tuple[Mapping[str, object], ...]
    input_marker_ids: frozenset[str]
    output_marker_ids: frozenset[str]


class LegacyWorkflowAuthoringService:
    """Own workflow-only migration into the normal native Cube import plan."""

    def __init__(
        self,
        *,
        resolver: SugarScriptCubeResolver,
        preparer: NativeCubeImportPreparer,
    ) -> None:
        """Bind exact catalog resolution and canonical native import preparation."""

        self._resolver = resolver
        self._preparer = preparer

    def compile(self, workflow: Mapping[str, object]) -> NativeWorkflowImportPlan:
        """Rebuild every managed Cube from its pin and saved semantic overrides."""

        nodes = _index_records(workflow.get("nodes"), "workflow node")
        instances = _read_instances(workflow, nodes)
        if not instances:
            raise LegacyWorkflowImportError(
                "Workflow contains no managed group-era Cubes"
            )
        native_subgraphs = _index_native_subgraphs(workflow)
        imports = tuple(
            self._prepare_instance(instance, native_subgraphs) for instance in instances
        )
        connections = _read_connections(workflow, instances, nodes)
        semantic_hash = hashlib.sha256(
            json.dumps(
                workflow,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=False,
            ).encode("utf-8")
        ).hexdigest()
        return NativeWorkflowImportPlan(semantic_hash, imports, connections)

    def _prepare_instance(
        self,
        instance: _LegacyInstance,
        native_subgraphs: Mapping[str, Mapping[str, object]],
    ) -> NativeCubeImport:
        """Resolve one exact Cube and apply saved values through stable node symbols."""

        try:
            document = self._resolver.resolve(instance.cube_id, instance.version)
        except ValueError as error:
            raise LegacyWorkflowImportError(
                f"Cube '{instance.alias}' exact version could not be resolved: {error}"
            ) from error
        payload = document.to_dict()
        implementation = _mapping(payload.get("implementation"), "implementation")
        runtime_nodes = materialize_nodes(document)
        definitions = document.implementation.definitions
        widget_names = surface_widget_names(payload)
        exact_subgraphs = _index_records(
            implementation.get("subgraphs"),
            "exact Cube subgraph",
        )
        for saved_node in instance.nodes:
            symbol = saved_symbol(saved_node)
            if not symbol:
                raise LegacyWorkflowImportError(
                    f"Cube '{instance.alias}' contains a node without a stable symbol"
                )
            target = runtime_nodes.get(symbol)
            if not isinstance(target, dict):
                raise LegacyWorkflowImportError(
                    f"Cube '{instance.alias}' exact version has no node symbol '{symbol}'"
                )
            overrides = reconcile_saved_node_values(
                saved_node,
                target,
                definitions,
                native_subgraphs,
                exact_subgraphs,
                widget_names.get(symbol),
            )
            inputs = target.setdefault("inputs", {})
            if not isinstance(inputs, dict):
                raise LegacyWorkflowImportError(
                    f"Cube '{instance.alias}' node '{symbol}' has invalid inputs"
                )
            inputs.update(deepcopy(overrides))
        implementation["nodes"] = runtime_nodes
        reconciled = CubeDocument.from_dict(payload)
        prepared = self._preparer.prepare(reconciled)
        return NativeCubeImport(
            instance.instance_id,
            instance.alias,
            _instance_bypassed(instance.metadata),
            prepared,
        )


def _read_instances(
    workflow: Mapping[str, object],
    nodes: Mapping[str, Mapping[str, object]],
) -> tuple[_LegacyInstance, ...]:
    """Read managed groups in saved order and bind their declared node identities."""

    result: list[_LegacyInstance] = []
    for index, group in enumerate(_records(workflow.get("groups"))):
        metadata = group.get("sugarcubes")
        if not isinstance(metadata, Mapping) or metadata.get("managed") is False:
            continue
        cube_id = _text(metadata.get("cube_id"))
        version = _text(metadata.get("cube_version"))
        if not cube_id or not version:
            raise LegacyWorkflowImportError(
                f"Managed group #{index + 1} lacks exact Cube identity"
            )
        instance_id = _text(metadata.get("instance_id")) or f"legacy-{index + 1}"
        alias = (
            _text(metadata.get("instance_alias"))
            or _text(metadata.get("default_alias"))
            or cube_id
        )
        node_ids = _identifiers(metadata.get("nodes"))
        owned: list[Mapping[str, object]] = []
        for node_id in node_ids:
            node = nodes.get(node_id)
            if node is None:
                raise LegacyWorkflowImportError(
                    f"Cube '{alias}' references missing node '{node_id}'"
                )
            owned.append(node)
        markers = metadata.get("markers")
        marker_map = markers if isinstance(markers, Mapping) else {}
        result.append(
            _LegacyInstance(
                instance_id,
                alias,
                cube_id,
                version,
                metadata,
                tuple(owned),
                frozenset(_identifiers(marker_map.get("inputs"))),
                frozenset(_identifiers(marker_map.get("outputs"))),
            )
        )
    return tuple(result)


def _read_connections(
    workflow: Mapping[str, object],
    instances: tuple[_LegacyInstance, ...],
    nodes: Mapping[str, Mapping[str, object]],
) -> tuple[CompiledCubeConnection, ...]:
    """Translate saved marker-to-marker edges into public Cube connections."""

    output_markers: dict[str, tuple[str, str]] = {}
    input_markers: dict[str, tuple[str, str]] = {}
    for instance in instances:
        for marker_id in instance.output_marker_ids:
            output_markers[marker_id] = (
                instance.instance_id,
                _marker_name(nodes.get(marker_id), marker_id),
            )
        for marker_id in instance.input_marker_ids:
            input_markers[marker_id] = (
                instance.instance_id,
                _marker_name(nodes.get(marker_id), marker_id),
            )
    result: list[CompiledCubeConnection] = []
    for raw_link in _sequence(workflow.get("links")):
        link = _parse_link(raw_link)
        if link is None:
            continue
        source = output_markers.get(link[0])
        target = input_markers.get(link[1])
        if source and target:
            result.append(
                CompiledCubeConnection(source[0], source[1], target[0], target[1])
            )
    return tuple(result)


def _parse_link(value: object) -> tuple[str, str] | None:
    """Read only the origin and target IDs from Comfy tuple or object links."""

    if isinstance(value, Mapping):
        origin = _identifier(value.get("origin_id"))
        target = _identifier(value.get("target_id"))
    elif isinstance(value, Sequence) and not isinstance(value, str | bytes):
        origin = _identifier(value[1]) if len(value) > 1 else ""
        target = _identifier(value[3]) if len(value) > 3 else ""
    else:
        return None
    return (origin, target) if origin and target else None


def _index_native_subgraphs(
    workflow: Mapping[str, object],
) -> dict[str, Mapping[str, object]]:
    """Index workflow-embedded native definitions used by legacy wrappers."""

    definitions = workflow.get("definitions")
    envelope = definitions if isinstance(definitions, Mapping) else {}
    return _index_records(envelope.get("subgraphs"), "native subgraph")


def _index_records(
    value: object,
    label: str,
) -> dict[str, Mapping[str, object]]:
    """Index records by graph ID while rejecting duplicates."""

    result: dict[str, Mapping[str, object]] = {}
    for record in _records(value):
        identity = _identifier(record.get("id"))
        if not identity:
            continue
        if identity in result:
            raise LegacyWorkflowImportError(f"Duplicate {label} id '{identity}'")
        result[identity] = record
    return result


def _records(value: object) -> list[Mapping[str, object]]:
    """Return only mapping entries from one untrusted sequence."""

    return [entry for entry in _sequence(value) if isinstance(entry, Mapping)]


def _sequence(value: object) -> Sequence[object]:
    """Return one non-string sequence or an empty tuple."""

    return (
        value
        if isinstance(value, Sequence) and not isinstance(value, str | bytes)
        else ()
    )


def _identifiers(value: object) -> list[str]:
    """Normalize graph identifiers from one untrusted sequence."""

    return [identity for entry in _sequence(value) if (identity := _identifier(entry))]


def _identifier(value: object) -> str:
    """Normalize one graph identifier without accepting booleans."""

    return (
        str(value)
        if isinstance(value, str | int) and not isinstance(value, bool)
        else ""
    )


def _integer(value: object) -> int:
    """Return one nonnegative graph slot or a negative sentinel."""

    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value >= 0
        else -1
    )


def _text(value: object) -> str:
    """Return one trimmed string."""

    return value.strip() if isinstance(value, str) else ""


def _mapping(value: object, label: str) -> dict[str, Any]:
    """Require one mutable mapping already owned by a copied Cube payload."""

    if not isinstance(value, dict):
        raise LegacyWorkflowImportError(f"Cube {label} is invalid")
    return value


def _marker_name(node: Mapping[str, object] | None, fallback: str) -> str:
    """Read one stable marker boundary name."""

    return saved_symbol(node) if node is not None and saved_symbol(node) else fallback


def _instance_bypassed(metadata: Mapping[str, object]) -> bool:
    """Read the only persisted group-level bypass forms without coercion."""

    return metadata.get("bypassed") is True or metadata.get("mode") == 4
