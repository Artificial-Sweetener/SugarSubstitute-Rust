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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Materialize explicit workflow-level value relationships without graph edges."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
import logging
import re
from typing import overload

from .stable_document_fields import (
    StableDocumentFieldValue,
    read_stable_document_field,
    write_stable_document_field,
)

COMPOSITION_METADATA_KEY = "sugarcubes_composition"
_EXACT_DIGITS = re.compile(r"-?(?:0|[1-9][0-9]{0,39})")
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
        if kind == "exact_integer":
            return _restore_exact_integer(relation.get("digits"), targets, documents)
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


def _restore_exact_integer(
    digits: object,
    targets: Sequence[StableFieldEndpoint],
    documents: Mapping[str, dict[str, object]],
) -> bool:
    """Give back the digits of a whole number that a browser could not hold.

    A page keeps numbers as doubles, so a 64-bit seed reaches it rounded. The
    relation records the seed's digits. Each target that still holds exactly
    that rounding receives the whole number; a target the person has since
    changed holds another number and is left alone.
    """

    if not isinstance(digits, str) or not _EXACT_DIGITS.fullmatch(digits):
        return False
    exact = int(digits)
    for target in targets:
        current = _read_value(documents, target)
        value = current.value if current.found else None
        if (
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and float(value) == float(exact)
        ):
            _write_value(documents, target, exact)
    return True


def restore_exact_prompt_integers(
    workflow: Mapping[str, object],
    prompt: Mapping[str, Mapping[str, object]],
) -> int:
    """Give a lowered prompt the whole numbers its workflow records as digits.

    A Cube that runs from its native graph takes its values from widgets the page
    holds as doubles, so its embedded document is not what executes. The same
    rule as for documents applies to the lowered nodes of the target field: an
    input still holding exactly the rounding of the recorded digits receives the
    whole number, and any other value is left alone.

    Returns:
        How many prompt inputs received their whole number.
    """

    restored = 0
    for relation in _relations(workflow):
        digits = relation.get("digits")
        if (
            relation.get("kind") != "exact_integer"
            or not isinstance(digits, str)
            or not _EXACT_DIGITS.fullmatch(digits)
        ):
            continue
        exact = int(digits)
        for target in _endpoints(relation.get("targets")):
            owner = f"{target.instance_id}:{target.node_symbol}"
            for node_id, node in prompt.items():
                # A node kept as a nested native graph lowers beneath its symbol.
                if node_id != owner and not node_id.startswith(f"{owner}:"):
                    continue
                inputs = node.get("inputs")
                value = (
                    inputs.get(target.input_name) if isinstance(inputs, dict) else None
                )
                if (
                    isinstance(inputs, dict)
                    and isinstance(value, (int, float))
                    and not isinstance(value, bool)
                    and value != exact
                    and float(value) == float(exact)
                ):
                    inputs[target.input_name] = exact
                    restored += 1
    return restored


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
    "restore_exact_prompt_integers",
]
