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
"""Read untrusted native workflow JSON into canonical SugarCubes values."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping, Sequence

from ..cube_model import (
    CubeDocument,
    CubeIdentityError,
    CubeSchemaError,
    parse_canonical_cube_id,
)
from .models import CanonicalWorkflow, CubeInstance, EmbeddedCubeDefinition
from .limits import (
    MAX_WORKFLOW_DEFINITIONS,
    MAX_WORKFLOW_LINKS,
    MAX_WORKFLOW_NODES,
)
from .semantic_hash import semantic_hash

_CUBE_KINDS = frozenset({"cube", "cube_draft"})


class CanonicalWorkflowError(ValueError):
    """Report one stable canonical-workflow validation failure."""

    def __init__(self, code: str, path: str, message: str) -> None:
        """Retain machine-readable failure identity beside the user message."""

        super().__init__(message)
        self.code = code
        self.path = path


def read_canonical_workflow(payload: object) -> CanonicalWorkflow:
    """Validate and copy one native Comfy workflow with embedded Cubes."""

    workflow = _require_mapping(payload, "$", "workflow.invalid_root")
    nodes = _require_sequence(
        workflow.get("nodes"), "$.nodes", "workflow.invalid_nodes"
    )
    _require_count(nodes, MAX_WORKFLOW_NODES, "$.nodes")
    links = workflow.get("links")
    if isinstance(links, Sequence) and not isinstance(links, (str, bytes, bytearray)):
        _require_count(links, MAX_WORKFLOW_LINKS, "$.links")
    definitions = _read_definitions(workflow)
    instances = _read_instances(nodes, definitions)
    copied = deepcopy(dict(workflow))
    return CanonicalWorkflow(
        semantic_hash=semantic_hash(copied),
        definitions=tuple(
            sorted(definitions.values(), key=lambda item: item.definition_id)
        ),
        instances=tuple(sorted(instances, key=lambda item: item.instance_id)),
        payload=copied,
    )


def _read_definitions(
    workflow: Mapping[str, object],
) -> dict[str, EmbeddedCubeDefinition]:
    """Validate every marked embedded definition and index it by native id."""

    envelope = workflow.get("definitions")
    if envelope is None:
        return {}
    definitions = _require_mapping(
        envelope, "$.definitions", "workflow.invalid_definitions"
    )
    raw_subgraphs = definitions.get("subgraphs")
    if raw_subgraphs is None:
        return {}
    subgraphs = _require_sequence(
        raw_subgraphs,
        "$.definitions.subgraphs",
        "workflow.invalid_definitions",
    )
    _require_count(
        subgraphs,
        MAX_WORKFLOW_DEFINITIONS,
        "$.definitions.subgraphs",
    )
    native_subgraphs = tuple(
        deepcopy(dict(value)) for value in subgraphs if isinstance(value, Mapping)
    )
    result: dict[str, EmbeddedCubeDefinition] = {}
    for index, raw_definition in enumerate(subgraphs):
        path = f"$.definitions.subgraphs[{index}]"
        definition = _require_mapping(
            raw_definition, path, "workflow.invalid_definition"
        )
        extra = definition.get("extra")
        if (
            not isinstance(extra, Mapping)
            or extra.get("sugarcubes_kind") not in _CUBE_KINDS
        ):
            continue
        definition_id = _require_string(
            definition.get("id"), f"{path}.id", "workflow.invalid_definition"
        )
        if definition_id in result:
            raise CanonicalWorkflowError(
                "workflow.duplicate_definition",
                f"{path}.id",
                f"Embedded definition id '{definition_id}' is duplicated.",
            )
        metadata = _require_mapping(
            extra.get("sugarcubes_cube"),
            f"{path}.extra.sugarcubes_cube",
            "workflow.invalid_cube_definition",
        )
        cube_id = _require_string(
            metadata.get("cube_id"),
            f"{path}.extra.sugarcubes_cube.cube_id",
            "workflow.invalid_cube_definition",
        )
        try:
            parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise CanonicalWorkflowError(
                "workflow.invalid_cube_identity",
                f"{path}.extra.sugarcubes_cube.cube_id",
                str(exc),
            ) from exc
        cube_version = _require_string(
            metadata.get("cube_version"),
            f"{path}.extra.sugarcubes_cube.cube_version",
            "workflow.invalid_cube_definition",
        )
        provenance_value = metadata.get("provenance")
        provenance = (
            deepcopy(dict(provenance_value))
            if isinstance(provenance_value, Mapping)
            else None
        )
        copied_definition = deepcopy(dict(definition))
        document = _read_embedded_document(
            extra,
            cube_id=cube_id,
            cube_version=cube_version,
            path=path,
        )
        result[definition_id] = EmbeddedCubeDefinition(
            definition_id=definition_id,
            cube_id=cube_id,
            cube_version=cube_version,
            default_alias=_optional_string(metadata.get("default_alias")),
            semantic_hash=semantic_hash(document or copied_definition),
            provenance=provenance,
            payload=copied_definition,
            document=document,
            native_subgraphs=native_subgraphs,
            native_node_definitions=_read_native_node_definitions(metadata, path),
        )
    return result


def _read_native_node_definitions(
    metadata: Mapping[str, object], path: str
) -> dict[str, object]:
    """Retain saved class schemas needed to execute native-only Cube state."""

    value = metadata.get("definitions")
    if value is None:
        return {}
    definitions = _require_mapping(
        value,
        f"{path}.extra.sugarcubes_cube.definitions",
        "workflow.invalid_cube_definition",
    )
    return deepcopy(dict(definitions))


def _read_embedded_document(
    extra: Mapping[str, object],
    *,
    cube_id: str,
    cube_version: str,
    path: str,
) -> dict[str, object] | None:
    """Validate optional portable content used for catalog comparison."""

    value = extra.get("sugarcubes_document")
    if value is None:
        return None
    document_path = f"{path}.extra.sugarcubes_document"
    raw = _require_mapping(value, document_path, "workflow.invalid_cube_document")
    try:
        document = CubeDocument.from_dict(raw)
    except CubeSchemaError as exc:
        raise CanonicalWorkflowError(
            "workflow.invalid_cube_document", document_path, str(exc)
        ) from exc
    if document.cube_id != cube_id or document.version != cube_version:
        return None
    return document.to_dict()


def _read_instances(
    nodes: Sequence[object],
    definitions: Mapping[str, EmbeddedCubeDefinition],
) -> list[CubeInstance]:
    """Validate marked Cube instances against authoritative embedded definitions."""

    instances: list[CubeInstance] = []
    seen_instance_ids: set[str] = set()
    for index, raw_node in enumerate(nodes):
        path = f"$.nodes[{index}]"
        node = _require_mapping(raw_node, path, "workflow.invalid_node")
        properties_value = node.get("properties")
        if not isinstance(properties_value, Mapping):
            continue
        kind = properties_value.get("sugarcubes_kind")
        if kind not in _CUBE_KINDS:
            continue
        metadata = _require_mapping(
            properties_value.get("sugarcubes_cube"),
            f"{path}.properties.sugarcubes_cube",
            "workflow.invalid_cube_instance",
        )
        node_id = _require_identifier(
            node.get("id"), f"{path}.id", "workflow.invalid_cube_instance"
        )
        definition_id = _require_string(
            node.get("type"), f"{path}.type", "workflow.invalid_cube_instance"
        )
        definition = definitions.get(definition_id)
        if definition is None:
            raise CanonicalWorkflowError(
                "workflow.missing_cube_definition",
                f"{path}.type",
                f"Cube node '{node_id}' references missing embedded definition '{definition_id}'.",
            )
        instance_id = _require_string(
            metadata.get("instance_id"),
            f"{path}.properties.sugarcubes_cube.instance_id",
            "workflow.invalid_cube_instance",
        )
        if instance_id in seen_instance_ids:
            raise CanonicalWorkflowError(
                "workflow.duplicate_instance",
                f"{path}.properties.sugarcubes_cube.instance_id",
                f"Cube instance id '{instance_id}' is duplicated.",
            )
        seen_instance_ids.add(instance_id)
        cube_id = _optional_string(metadata.get("cube_id")) or definition.cube_id
        cube_version = (
            _optional_string(metadata.get("cube_version")) or definition.cube_version
        )
        if cube_id != definition.cube_id or cube_version != definition.cube_version:
            raise CanonicalWorkflowError(
                "workflow.cube_identity_mismatch",
                f"{path}.properties.sugarcubes_cube",
                f"Cube instance '{instance_id}' identity disagrees with embedded definition '{definition_id}'.",
            )
        instances.append(
            CubeInstance(
                instance_id=instance_id,
                node_id=node_id,
                definition_id=definition_id,
                cube_id=cube_id,
                cube_version=cube_version,
                instance_alias=_optional_string(metadata.get("instance_alias"))
                or definition.default_alias,
                payload=deepcopy(dict(node)),
                execution_mode=_read_execution_mode(node.get("mode"), f"{path}.mode"),
            )
        )
    return instances


def _read_execution_mode(value: object, path: str) -> int:
    """Validate the bounded native node mode that changes Cube execution."""

    if value is None:
        return 0
    if isinstance(value, bool) or not isinstance(value, int) or value not in range(5):
        raise CanonicalWorkflowError(
            "workflow.invalid_cube_execution_mode",
            path,
            f"Workflow value at '{path}' must be a native execution mode from 0 through 4.",
        )
    return value


def _require_mapping(value: object, path: str, code: str) -> Mapping[str, object]:
    """Return one string-keyed mapping or raise a located diagnostic."""

    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise CanonicalWorkflowError(
            code, path, f"Workflow value at '{path}' must be an object."
        )
    return value


def _require_sequence(value: object, path: str, code: str) -> Sequence[object]:
    """Return one JSON array without accepting text as a sequence."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise CanonicalWorkflowError(
            code, path, f"Workflow value at '{path}' must be an array."
        )
    return value


def _require_count(values: Sequence[object], maximum: int, path: str) -> None:
    """Reject hostile collection sizes before copying or recursive traversal."""

    if len(values) > maximum:
        raise CanonicalWorkflowError(
            "workflow.limit_exceeded",
            path,
            f"Workflow collection at '{path}' exceeds the limit of {maximum} entries.",
        )


def _require_string(value: object, path: str, code: str) -> str:
    """Return one required trimmed string or raise a located diagnostic."""

    normalized = _optional_string(value)
    if not normalized:
        raise CanonicalWorkflowError(
            code, path, f"Workflow value at '{path}' must be a non-empty string."
        )
    return normalized


def _require_identifier(value: object, path: str, code: str) -> str:
    """Normalize a persisted Comfy string or integer identifier."""

    if isinstance(value, bool) or not isinstance(value, (str, int)):
        raise CanonicalWorkflowError(
            code, path, f"Workflow value at '{path}' must be an identifier."
        )
    normalized = str(value).strip()
    if not normalized:
        raise CanonicalWorkflowError(
            code, path, f"Workflow value at '{path}' must be an identifier."
        )
    return normalized


def _optional_string(value: object) -> str:
    """Normalize optional metadata text without inventing a value."""

    return value.strip() if isinstance(value, str) else ""
