#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Normalize saved native Cube state through exact versioned documents."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from typing import Protocol

from ..cube_model import CubeDocument
from ..cube_model.merge import materialize_nodes
from ..cube_model.picker_fields import widget_input_names
from ..cube_model.subgraph_boundary_widgets import index_boundary_widget_targets
from ..cube_model.widget_values import WORKFLOW_WIDGET_VALUES_KEY
from ..execution.portable_boundary_lowering import document_boundary_ports
from ..workflow import read_canonical_workflow
from .legacy_workflow_values import (
    LegacyWorkflowImportError,
    reconcile_saved_node_values,
    saved_symbol,
    surface_widget_names,
)

_EXACT_NODE_KEYS = frozenset({"class_type", "inputs", "label", "original_id"})


class ExactCubeDocumentResolver(Protocol):
    """Resolve one exact catalog document for saved native reconciliation."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return the exact requested Cube document or raise a value error."""


class NativeWorkflowNormalizationError(ValueError):
    """Reject native Cube state that cannot be normalized without guessing."""


class NativeWorkflowNormalizer:
    """Attach name-addressed values and portable documents to native Cubes."""

    def __init__(self, resolver: ExactCubeDocumentResolver) -> None:
        """Bind exact catalog resolution at the normalization boundary."""

        self._resolver = resolver

    def normalize(self, workflow_value: object) -> dict[str, object]:
        """Return a complete detached graph preserving all original graph data."""

        workflow = read_canonical_workflow(workflow_value)
        normalized = deepcopy(dict(workflow.payload))
        definitions = _definition_index(normalized)
        native_subgraphs = _definition_index(normalized)
        documents_by_definition: dict[str, CubeDocument] = {}
        for definition in workflow.definitions:
            target = definitions.get(definition.definition_id)
            if target is None:
                continue
            document = self._exact_document(definition)
            reconciled = _reconcile_definition(
                target,
                document,
                embedded_document=getattr(definition, "document"),
                native_subgraphs=native_subgraphs,
            )
            extra = target.setdefault("extra", {})
            if not isinstance(extra, dict):
                raise NativeWorkflowNormalizationError(
                    f"Cube definition '{definition.definition_id}' has invalid extra data."
                )
            extra["sugarcubes_document"] = reconciled.to_dict()
            definition_id = str(getattr(definition, "definition_id"))
            documents_by_definition[definition_id] = reconciled
            inputs, outputs = document_boundary_ports(reconciled)
            target["inputs"] = deepcopy(list(inputs))
            target["outputs"] = deepcopy(list(outputs))
        _reconcile_instance_boundaries(normalized, documents_by_definition)
        return normalized

    def _exact_document(self, definition: object) -> CubeDocument:
        """Resolve pinned Cubes while retaining embedded authority for drafts."""

        embedded = getattr(definition, "document")
        payload = getattr(definition, "payload")
        if _is_cube_draft(payload) and isinstance(embedded, Mapping):
            return CubeDocument.from_dict(embedded)
        cube_id = str(getattr(definition, "cube_id"))
        cube_version = str(getattr(definition, "cube_version"))
        try:
            return self._resolver.resolve(cube_id, cube_version)
        except ValueError as error:
            raise NativeWorkflowNormalizationError(
                f"Cube '{cube_id}' version '{cube_version}' cannot be normalized: {error}"
            ) from error


def _is_cube_draft(payload: object) -> bool:
    """Return whether one native definition is an unpublished Cube draft."""

    if not isinstance(payload, Mapping):
        return False
    extra = payload.get("extra")
    return isinstance(extra, Mapping) and extra.get("sugarcubes_kind") == "cube_draft"


def _reconcile_definition(
    native_definition: dict[str, object],
    exact_document: CubeDocument,
    *,
    embedded_document: object,
    native_subgraphs: Mapping[str, Mapping[str, object]],
) -> CubeDocument:
    """Reconcile saved native nodes into exact portable and name-addressed state."""

    payload = exact_document.to_dict()
    implementation = payload.get("implementation")
    if not isinstance(implementation, dict):
        raise NativeWorkflowNormalizationError("Exact Cube implementation is invalid.")
    runtime_nodes = materialize_nodes(exact_document)
    _apply_embedded_instance_state(
        runtime_nodes,
        exact_document=exact_document,
        embedded_document=embedded_document,
    )
    definitions = exact_document.implementation.definitions
    exact_subgraphs = _record_index(implementation.get("subgraphs"))
    widget_names = surface_widget_names(payload)
    for saved_node in _records(native_definition.get("nodes")):
        symbol = saved_symbol(saved_node)
        if not symbol:
            continue
        target = runtime_nodes.get(symbol)
        if not isinstance(target, dict):
            raise NativeWorkflowNormalizationError(
                f"Exact Cube has no node symbol '{symbol}'."
            )
        versioned_names = widget_names.get(symbol)
        if versioned_names is None:
            target_class_type = target.get("class_type")
            target_definition = (
                definitions.get(target_class_type)
                if isinstance(target_class_type, str)
                else None
            )
            versioned_names = (
                widget_input_names(target_definition)
                if isinstance(target_definition, Mapping)
                else None
            )
        try:
            values = reconcile_saved_node_values(
                saved_node,
                target,
                definitions,
                native_subgraphs,
                exact_subgraphs,
                versioned_names,
            )
        except LegacyWorkflowImportError as error:
            raise NativeWorkflowNormalizationError(str(error)) from error
        if isinstance(saved_node, dict):
            saved_node[WORKFLOW_WIDGET_VALUES_KEY] = deepcopy(values)
        inputs = target.setdefault("inputs", {})
        if not isinstance(inputs, dict):
            raise NativeWorkflowNormalizationError(
                f"Exact Cube node '{symbol}' has invalid inputs."
            )
        inputs.update(deepcopy(values))
    implementation["nodes"] = runtime_nodes
    return CubeDocument.from_dict(payload)


def _apply_embedded_instance_state(
    runtime_nodes: dict[str, dict[str, object]],
    *,
    exact_document: CubeDocument,
    embedded_document: object,
) -> None:
    """Overlay stable saved fields and opaque host metadata onto exact nodes."""

    if not isinstance(embedded_document, Mapping):
        return
    try:
        embedded = CubeDocument.from_dict(embedded_document)
    except ValueError as error:
        raise NativeWorkflowNormalizationError(
            "Embedded Cube instance document is invalid and cannot be discarded."
        ) from error
    if (
        embedded.cube_id != exact_document.cube_id
        or embedded.version != exact_document.version
    ):
        raise NativeWorkflowNormalizationError(
            "Embedded Cube instance identity does not match its exact definition."
        )
    embedded_runtime_nodes = embedded.implementation.nodes
    definitions = exact_document.implementation.definitions
    exact_subgraphs = _record_index(exact_document.implementation.subgraphs)
    for symbol, target in runtime_nodes.items():
        source = embedded_runtime_nodes.get(symbol)
        source_inputs = source.get("inputs") if isinstance(source, Mapping) else None
        target_inputs = target.get("inputs") if isinstance(target, dict) else None
        class_type = target.get("class_type") if isinstance(target, Mapping) else None
        definition = (
            definitions.get(class_type) if isinstance(class_type, str) else None
        )
        if isinstance(source_inputs, Mapping) and isinstance(target_inputs, dict):
            exact_subgraph = (
                exact_subgraphs.get(class_type) if isinstance(class_type, str) else None
            )
            stable_names = (
                tuple(index_boundary_widget_targets(exact_subgraph))
                if isinstance(exact_subgraph, Mapping)
                else (
                    widget_input_names(definition)
                    if isinstance(definition, Mapping)
                    else ()
                )
            )
            for input_name in stable_names:
                if input_name in source_inputs:
                    target_inputs[input_name] = deepcopy(source_inputs[input_name])
    for symbol, target in runtime_nodes.items():
        source = embedded.implementation.nodes.get(symbol)
        if not isinstance(source, Mapping):
            continue
        for key, value in source.items():
            if key not in _EXACT_NODE_KEYS:
                target[key] = deepcopy(value)


def _definition_index(
    workflow: Mapping[str, object],
) -> dict[str, dict[str, object]]:
    """Index mutable native subgraph definitions by stable identifier."""

    envelope = workflow.get("definitions")
    values = envelope.get("subgraphs") if isinstance(envelope, Mapping) else None
    return {
        str(value["id"]): value
        for value in _records(values)
        if isinstance(value, dict)
        and isinstance(value.get("id"), str | int)
        and not isinstance(value.get("id"), bool)
    }


def _reconcile_instance_boundaries(
    workflow: dict[str, object],
    documents_by_definition: Mapping[str, CubeDocument],
) -> None:
    """Restore exact public sockets only on recognized Cube instances."""

    for node in _records(workflow.get("nodes")):
        if not isinstance(node, dict):
            continue
        properties = node.get("properties")
        if not isinstance(properties, Mapping) or properties.get(
            "sugarcubes_kind"
        ) not in {"cube", "cube_draft"}:
            continue
        definition_id = node.get("type")
        document = (
            documents_by_definition.get(str(definition_id))
            if isinstance(definition_id, str | int)
            and not isinstance(definition_id, bool)
            else None
        )
        if document is None:
            continue
        inputs, outputs = document_boundary_ports(document)
        node["inputs"] = deepcopy(list(inputs))
        node["outputs"] = deepcopy(list(outputs))


def _record_index(value: object) -> dict[str, Mapping[str, object]]:
    """Index immutable records by graph identifier."""

    return {
        str(item["id"]): item
        for item in _records(value)
        if isinstance(item.get("id"), str | int)
        and not isinstance(item.get("id"), bool)
    }


def _records(value: object) -> list[Mapping[str, object]]:
    """Return mapping entries from one untrusted serialized sequence."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return []
    return [item for item in value if isinstance(item, Mapping)]


__all__ = [
    "ExactCubeDocumentResolver",
    "NativeWorkflowNormalizationError",
    "NativeWorkflowNormalizer",
]
