#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Materialize explicit workflow-level value relationships without graph edges."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
import logging
from typing import overload

from .stable_document_fields import (
    StableDocumentFieldValue,
    read_stable_document_field,
    write_stable_document_field,
)

COMPOSITION_METADATA_KEY = "sugarcubes_composition"
_logger = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class StableFieldEndpoint:
    """Address one Cube-owned value by stable workflow and document identities."""

    instance_id: str
    node_symbol: str
    input_name: str


class ComposedValueMaterializer:
    """Apply explicit links and overrides to a detached canonical workflow."""

    @overload
    def materialize(
        self, workflow_value: Mapping[str, object]
    ) -> dict[str, object]: ...

    @overload
    def materialize(self, workflow_value: object) -> object: ...

    def materialize(self, workflow_value: object) -> object:
        """Return a detached graph with recognized value relations projected."""

        if not isinstance(workflow_value, Mapping):
            return workflow_value
        workflow = deepcopy(dict(workflow_value))
        relations = _relations(workflow)
        if not relations:
            return workflow
        documents = _documents_by_instance(workflow)
        ignored = 0
        for relation in relations:
            if not self._apply_relation(relation, documents):
                ignored += 1
        if ignored:
            _logger.warning(
                "Ignored stale or malformed SugarCubes value relations count=%d",
                ignored,
            )
        return workflow

    def _apply_relation(
        self,
        relation: Mapping[str, object],
        documents: Mapping[str, dict[str, object]],
    ) -> bool:
        """Apply one recognized relation while leaving invalid metadata inert."""

        targets = _endpoints(relation.get("targets"))
        if not targets:
            return False
        kind = relation.get("kind")
        if kind == "field_link":
            source = _endpoint(relation.get("source"))
            if source is None:
                return False
            resolved = _read_value(documents, source)
            if not resolved.found:
                return False
            value = resolved.value
        elif kind == "global_override" and "value" in relation:
            value = relation["value"]
        else:
            return False
        if any(not _read_value(documents, target).found for target in targets):
            return False
        return all(_write_value(documents, target, value) for target in targets)


def _relations(workflow: Mapping[str, object]) -> tuple[Mapping[str, object], ...]:
    """Return schema-one relation records without rejecting unrelated workflows."""

    extra = workflow.get("extra")
    composition = (
        extra.get(COMPOSITION_METADATA_KEY) if isinstance(extra, Mapping) else None
    )
    if not isinstance(composition, Mapping) or composition.get("schema_version") != 1:
        return ()
    values = composition.get("value_relations")
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        return ()
    return tuple(value for value in values if isinstance(value, Mapping))


def _documents_by_instance(
    workflow: Mapping[str, object],
) -> dict[str, dict[str, object]]:
    """Index mutable embedded documents by stable Cube instance identity."""

    definitions_value = workflow.get("definitions")
    subgraphs = (
        definitions_value.get("subgraphs")
        if isinstance(definitions_value, Mapping)
        else None
    )
    definitions = {
        str(value.get("id")): value
        for value in _mapping_records(subgraphs)
        if isinstance(value, dict)
    }
    result: dict[str, dict[str, object]] = {}
    for node in _mapping_records(workflow.get("nodes")):
        properties = node.get("properties")
        cube = (
            properties.get("sugarcubes_cube")
            if isinstance(properties, Mapping)
            else None
        )
        instance_id = cube.get("instance_id") if isinstance(cube, Mapping) else None
        definition = definitions.get(str(node.get("type")))
        extra = definition.get("extra") if isinstance(definition, Mapping) else None
        document = (
            extra.get("sugarcubes_document") if isinstance(extra, Mapping) else None
        )
        if isinstance(instance_id, str) and isinstance(document, dict):
            result[instance_id] = document
    return result


def _endpoint(value: object) -> StableFieldEndpoint | None:
    """Parse one stable endpoint without coercing ambiguous identities."""

    if not isinstance(value, Mapping):
        return None
    parts = tuple(
        value.get(key) for key in ("instance_id", "node_symbol", "input_name")
    )
    if not all(isinstance(part, str) and part.strip() for part in parts):
        return None
    return StableFieldEndpoint(*(str(part).strip() for part in parts))


def _endpoints(value: object) -> tuple[StableFieldEndpoint, ...]:
    """Parse a non-empty endpoint sequence or return no usable targets."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return ()
    endpoints = tuple(_endpoint(item) for item in value)
    if not endpoints or any(endpoint is None for endpoint in endpoints):
        return ()
    return tuple(endpoint for endpoint in endpoints if endpoint is not None)


def _read_value(
    documents: Mapping[str, dict[str, object]],
    endpoint: StableFieldEndpoint,
) -> StableDocumentFieldValue:
    """Read one authoritative instance input by stable field identity."""

    document = documents.get(endpoint.instance_id)
    if document is None:
        return StableDocumentFieldValue(False)
    return read_stable_document_field(
        document,
        endpoint.node_symbol,
        endpoint.input_name,
    )


def _write_value(
    documents: Mapping[str, dict[str, object]],
    endpoint: StableFieldEndpoint,
    value: object,
) -> bool:
    """Write one detached execution input without changing authored presets."""

    document = documents[endpoint.instance_id]
    return write_stable_document_field(
        document,
        endpoint.node_symbol,
        endpoint.input_name,
        value,
    )


def _mapping_records(value: object) -> tuple[Mapping[str, object], ...]:
    """Return mapping entries from one JSON-like array."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return ()
    return tuple(item for item in value if isinstance(item, Mapping))


__all__ = [
    "COMPOSITION_METADATA_KEY",
    "ComposedValueMaterializer",
    "StableFieldEndpoint",
]
