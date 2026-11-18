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

"""Rebuild a document-carrying Cube workflow as a native workflow import plan.

SugarSubstitute's Python application saved workflows whose Cube definitions
hold their whole Cube document and no native nodes, and whose shared values
live in workflow-level relations. Execution already reads that form; this
service turns it into the plan the page materializes as native Cubes.
"""

from __future__ import annotations

from copy import deepcopy

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Protocol

from ..cube_model import CubeDocument
from ..execution.cube_document_lowering import unreplaceable_bypassed_nodes
from ..language.compiler_models import CompiledCubeConnection
from ..workflow import (
    ComposedValueMaterializer,
    CubeInstance,
    read_canonical_workflow,
)
from .native_workflow_models import NativeCubeImport, NativeWorkflowImportPlan
from .native_workflow_service import NativeCubeImportPreparer

MAX_BROWSER_INTEGER = 2**53 - 1
"""The largest whole number a browser holds exactly."""

_BYPASSED = 4
_INPUT_PREFIX = "input."
_OUTPUT_PREFIX = "output."


class ComposedWorkflowImportError(ValueError):
    """Report a document-carrying workflow that cannot be rebuilt."""


class WorkflowNormalizer(Protocol):
    """Complete a workflow's embedded Cube documents from its own content."""

    def normalize(self, workflow_value: object) -> dict[str, object]:
        """Return a detached workflow whose definitions carry exact documents."""


@dataclass(frozen=True)
class ExactIntegerField:
    """Name one whole-number input whose value a browser cannot hold exactly."""

    instance_id: str
    node_symbol: str
    input_name: str
    digits: str


@dataclass(frozen=True)
class ComposedWorkflowImport:
    """Pair the native plan with the exact digits its page-side values lose."""

    plan: NativeWorkflowImportPlan
    exact_integers: tuple[ExactIntegerField, ...]


class ComposedWorkflowAuthoringService:
    """Own the import of workflows whose Cubes are carried as documents."""

    def __init__(
        self,
        *,
        normalizer: WorkflowNormalizer,
        preparer: NativeCubeImportPreparer,
    ) -> None:
        """Bind workflow normalization and canonical native import preparation."""

        self._normalizer = normalizer
        self._preparer = preparer

    @staticmethod
    def recognizes(workflow: Mapping[str, object]) -> bool:
        """Report whether every Cube definition is a document with no native nodes.

        A workflow the page saved carries native nodes beside each document and
        loads as it is; only the document-only form needs rebuilding.
        """

        definitions = workflow.get("definitions")
        subgraphs = (
            definitions.get("subgraphs") if isinstance(definitions, Mapping) else None
        )
        if not isinstance(subgraphs, Sequence) or isinstance(subgraphs, (str, bytes)):
            return False
        documents = 0
        for subgraph in subgraphs:
            extra = subgraph.get("extra") if isinstance(subgraph, Mapping) else None
            if not isinstance(extra, Mapping) or not isinstance(
                extra.get("sugarcubes_document"), Mapping
            ):
                continue
            # The Python application wrote an explicit empty node list.
            if subgraph.get("nodes") != []:
                return False
            documents += 1
        return documents > 0

    def compile(self, workflow: Mapping[str, object]) -> ComposedWorkflowImport:
        """Rebuild every Cube from its document with the workflow's relations applied.

        Links and overrides are written into the documents first, so each Cube
        arrives holding the values the workflow ran with. Cubes are ordered as
        the stack showed them, left to right, and each Cube input is fed by the
        nearest earlier Cube that offers an output of the same name.
        """

        try:
            normalized = self._normalizer.normalize(workflow)
            materialized = ComposedValueMaterializer().materialize(normalized)
            canonical = read_canonical_workflow(materialized)
        except ValueError as error:
            raise ComposedWorkflowImportError(str(error)) from error
        if not canonical.instances:
            raise ComposedWorkflowImportError("Workflow contains no Cubes")
        documents = {
            definition.definition_id: definition.document
            for definition in canonical.definitions
        }
        ordered = sorted(
            enumerate(canonical.instances),
            key=lambda item: (*_stack_position(item[1]), item[0]),
        )
        imports: list[NativeCubeImport] = []
        exact: list[ExactIntegerField] = []
        staged: list[tuple[CubeInstance, Mapping[str, object]]] = []
        for _, instance in ordered:
            document = documents.get(instance.definition_id)
            if not isinstance(document, Mapping):
                raise ComposedWorkflowImportError(
                    f"Cube '{instance.instance_alias}' carries no Cube document"
                )
            document = _with_needed_nodes_running(document)
            try:
                cube = CubeDocument.from_dict(document)
            except ValueError as error:
                raise ComposedWorkflowImportError(
                    f"Cube '{instance.instance_alias}' document is invalid: {error}"
                ) from error
            exact.extend(exact_integer_fields(instance.instance_id, document))
            imports.append(
                NativeCubeImport(
                    instance.instance_id,
                    instance.instance_alias,
                    instance.execution_mode == _BYPASSED,
                    self._preparer.prepare(cube),
                )
            )
            staged.append((instance, document))
        return ComposedWorkflowImport(
            NativeWorkflowImportPlan(
                canonical.semantic_hash,
                tuple(imports),
                _stack_connections(staged),
            ),
            tuple(exact),
        )


def _with_needed_nodes_running(
    document: Mapping[str, object],
) -> Mapping[str, object]:
    """Run the bypassed nodes whose output a running node needs and nothing supplies.

    A Cube the Python application saved can hold a prompt linked to an earlier
    Cube's prompt beside a bypassed prompt encoder. The link gives the Cube its
    prompt text; with the encoder bypassed its sampler receives no conditioning
    and fails. Such an encoder is set to run, so it encodes the linked prompt
    with the CLIP the Cube inherits. Nodes whose bypass passes an input through,
    or whose outputs are inherited, keep their bypass.
    """

    implementation = document.get("implementation")
    if not isinstance(implementation, Mapping):
        return document
    nodes = implementation.get("nodes")
    definitions = implementation.get("definitions")
    if not isinstance(nodes, Mapping) or not isinstance(definitions, Mapping):
        return document
    usable = {
        str(symbol): node for symbol, node in nodes.items() if isinstance(node, Mapping)
    }
    revived = unreplaceable_bypassed_nodes(usable, definitions)
    if not revived:
        return document
    running = {
        symbol: (
            {key: value for key, value in node.items() if key != "mode"}
            if symbol in revived
            else node
        )
        for symbol, node in usable.items()
    }
    return {
        **document,
        "implementation": {**implementation, "nodes": deepcopy(running)},
    }


def _stack_position(instance: CubeInstance) -> tuple[float, float]:
    """Read where the stack drew a Cube; a Cube without a position sorts first."""

    position = instance.payload.get("pos")
    if (
        isinstance(position, Sequence)
        and not isinstance(position, (str, bytes))
        and len(position) >= 2
        and all(
            isinstance(value, (int, float)) and not isinstance(value, bool)
            for value in position[:2]
        )
    ):
        return float(position[0]), float(position[1])
    return 0.0, 0.0


def _boundary_names(document: Mapping[str, object], key: str) -> tuple[str, ...]:
    """Read a document's public input or output names in authored order."""

    implementation = document.get("implementation")
    boundary = implementation.get(key) if isinstance(implementation, Mapping) else None
    return tuple(boundary) if isinstance(boundary, Mapping) else ()


def _stack_connections(
    staged: Sequence[tuple[CubeInstance, Mapping[str, object]]],
) -> tuple[CompiledCubeConnection, ...]:
    """Feed each Cube input from the nearest earlier output of the same name."""

    connections: list[CompiledCubeConnection] = []
    for index, (target, document) in enumerate(staged):
        for name in _boundary_names(document, "inputs"):
            if not name.startswith(_INPUT_PREFIX):
                continue
            wanted = _OUTPUT_PREFIX + name[len(_INPUT_PREFIX) :]
            for source, source_document in reversed(staged[:index]):
                if wanted in _boundary_names(source_document, "outputs"):
                    connections.append(
                        CompiledCubeConnection(
                            source.instance_id,
                            wanted,
                            target.instance_id,
                            name,
                        )
                    )
                    break
    return tuple(connections)


def exact_integer_fields(
    instance_id: str, document: Mapping[str, object]
) -> tuple[ExactIntegerField, ...]:
    """Collect whole-number inputs beyond what a browser holds exactly."""

    implementation = document.get("implementation")
    nodes = implementation.get("nodes") if isinstance(implementation, Mapping) else None
    if not isinstance(nodes, Mapping):
        return ()
    fields: list[ExactIntegerField] = []
    for symbol, node in nodes.items():
        inputs = node.get("inputs") if isinstance(node, Mapping) else None
        if not isinstance(inputs, Mapping):
            continue
        for name, value in inputs.items():
            if (
                isinstance(value, int)
                and not isinstance(value, bool)
                and abs(value) > MAX_BROWSER_INTEGER
            ):
                fields.append(
                    ExactIntegerField(instance_id, str(symbol), str(name), str(value))
                )
    return tuple(fields)


__all__ = [
    "ComposedWorkflowAuthoringService",
    "ComposedWorkflowImport",
    "ComposedWorkflowImportError",
    "ExactIntegerField",
    "exact_integer_fields",
    "MAX_BROWSER_INTEGER",
]
