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
"""Describe a native Cube workflow as the project script that would rebuild it.

A workflow holds every value of every node; a project script holds only what a Cube exposes to
the person, named as they see it. This owner selects those fields, names them, and orders the
Cubes, so the language's writer needs no knowledge of workflows or Cube documents.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from ..cube_model import CubeDocument
from ..cube_model.subgraph_boundary_widgets import index_boundary_widget_targets
from ..execution import ConnectionOrigin
from ..language.substitute_writer import (
    ScriptConnection,
    ScriptCube,
    ScriptField,
    ScriptNode,
    SubstituteScript,
)
from ..workflow_analysis import CubeGraphAnalysis

_DOCUMENT_KEY = "sugarcubes_document"
_SAVE_OUTPUTS_KEY = "sugarcubes_save_outputs"
_BYPASS_MODE = 4


class WorkflowScriptProjectionError(ValueError):
    """Report a workflow that cannot be written as one project script."""


def project_workflow_script(analysis: CubeGraphAnalysis) -> SubstituteScript:
    """Return the script model of one analysed workflow.

    Cubes appear in stack order. A Cube persists every field its surface exposes, with the
    value the workflow holds now; whether that value equals a default plays no part, so a saved
    project keeps its values when a Cube's defaults later change.
    """

    documents = _documents_by_definition(analysis.normalized_workflow)
    instances = {instance.instance_id: instance for instance in analysis.instances}
    ordered = [
        instances[instance_id]
        for segment in analysis.segments
        for instance_id in segment.instance_ids
        if instance_id in instances
    ]
    ordered.extend(
        instance for instance in analysis.instances if instance not in ordered
    )
    aliases: set[str] = set()
    cubes: list[ScriptCube] = []
    for instance in ordered:
        if instance.instance_alias in aliases:
            raise WorkflowScriptProjectionError(
                f"Two Cubes are named '{instance.instance_alias}'; a project script "
                "names each Cube once."
            )
        aliases.add(instance.instance_alias)
        document = documents.get(str(instance.payload.get("type")))
        if document is None:
            raise WorkflowScriptProjectionError(
                f"Cube '{instance.instance_alias}' carries no Cube document."
            )
        properties = instance.payload.get("properties")
        cubes.append(
            ScriptCube(
                alias=instance.instance_alias,
                cube_id=instance.cube_id,
                version_pin=instance.cube_version or None,
                nodes=_script_nodes(document),
                bypassed=instance.execution_mode == _BYPASS_MODE,
                save_outputs=not (
                    isinstance(properties, Mapping)
                    and properties.get(_SAVE_OUTPUTS_KEY) is False
                ),
            )
        )
    alias_of = {instance.instance_id: instance.instance_alias for instance in ordered}
    return SubstituteScript(
        cubes=tuple(cubes),
        connections=tuple(
            ScriptConnection(
                alias_of[edge.source.instance_id],
                edge.source.binding,
                alias_of[edge.target.instance_id],
                edge.target.binding,
            )
            for edge in analysis.edges
            if edge.origin is ConnectionOrigin.EXPLICIT
            and edge.source.instance_id in alias_of
            and edge.target.instance_id in alias_of
        ),
    )


def _documents_by_definition(workflow: Mapping[str, object]) -> dict[str, CubeDocument]:
    """Read each Cube definition's embedded document, keyed by the definition's identity."""

    definitions = workflow.get("definitions")
    subgraphs = (
        definitions.get("subgraphs") if isinstance(definitions, Mapping) else None
    )
    documents: dict[str, CubeDocument] = {}
    if not isinstance(subgraphs, Sequence) or isinstance(subgraphs, (str, bytes)):
        return documents
    for subgraph in subgraphs:
        if not isinstance(subgraph, Mapping):
            continue
        extra = subgraph.get("extra")
        payload = extra.get(_DOCUMENT_KEY) if isinstance(extra, Mapping) else None
        identity = subgraph.get("id")
        if isinstance(payload, Mapping) and isinstance(identity, str):
            documents[identity] = CubeDocument.from_dict(payload)
    return documents


def _script_nodes(document: CubeDocument) -> tuple[ScriptNode, ...]:
    """Select and name the fields one Cube document exposes, in node then input order."""

    implementation = document.implementation
    exposed = _exposed_fields(document)
    surface_labels = {
        (control.symbol, control.input_name): control.label
        for control in document.surface.controls
        if control.label.strip()
    }
    nodes: list[ScriptNode] = []
    for key, node in implementation.nodes.items():
        inputs = node.get("inputs")
        fields = tuple(
            ScriptField(
                input_key=input_key,
                input_label=_input_label(
                    implementation.definitions,
                    implementation.subgraphs,
                    node,
                    input_key,
                    surface_labels.get((key, input_key)),
                ),
                value=value,
            )
            for input_key, value in (
                inputs.items() if isinstance(inputs, Mapping) else ()
            )
            if isinstance(input_key, str) and (key, input_key) in exposed
        )
        label = node.get("label")
        mode = node.get("mode")
        nodes.append(
            ScriptNode(
                key=key,
                label=(
                    label.strip() if isinstance(label, str) and label.strip() else key
                ),
                fields=fields,
                mode=(
                    mode if isinstance(mode, int) and not isinstance(mode, bool) else 0
                ),
            )
        )
    return tuple(nodes)


def _exposed_fields(document: CubeDocument) -> frozenset[tuple[str, str]]:
    """Name the fields a person edits on this Cube.

    Those are its surface controls and the widget-backed public inputs of its wrapper nodes,
    without any field the Cube's own boundary supplies from another Cube.
    """

    implementation = document.implementation
    fields = {
        (control.symbol, control.input_name) for control in document.surface.controls
    }
    wrappers = {
        subgraph.get("id"): subgraph
        for subgraph in implementation.subgraphs
        if isinstance(subgraph, Mapping)
    }
    for key, node in implementation.nodes.items():
        wrapper = (
            wrappers.get(node.get("class_type")) if isinstance(node, Mapping) else None
        )
        if isinstance(wrapper, Mapping):
            fields.update(
                (key, name) for name in index_boundary_widget_targets(wrapper)
            )
    for binding in implementation.inputs.values():
        targets = binding.get("targets") if isinstance(binding, Mapping) else binding
        if not isinstance(targets, Sequence) or isinstance(targets, (str, bytes)):
            continue
        for target in targets:
            if (
                isinstance(target, Sequence)
                and not isinstance(target, (str, bytes))
                and len(target) == 2
            ):
                fields.discard((str(target[0]), str(target[1])))
    return frozenset(fields)


def _input_label(
    definitions: Mapping[str, object],
    subgraphs: Sequence[object],
    node: Mapping[str, object],
    input_key: str,
    surface_label: str | None,
) -> str:
    """Name one input as the person sees it.

    The surface's own label wins, then the label its node definition declares, then the label
    of the wrapper's public input, and last the input's key.
    """

    if surface_label:
        return surface_label.strip()
    class_type = node.get("class_type")
    definition = definitions.get(class_type) if isinstance(class_type, str) else None
    declared = _definition_label(definition, input_key)
    if declared:
        return declared
    for subgraph in subgraphs:
        if not isinstance(subgraph, Mapping) or subgraph.get("id") != class_type:
            continue
        entries = subgraph.get("inputs")
        if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
            continue
        for entry in entries:
            if isinstance(entry, Mapping) and entry.get("name") == input_key:
                label = entry.get("label")
                if isinstance(label, str) and label.strip():
                    return label.strip()
    return input_key


def _definition_label(definition: object, input_key: str) -> str | None:
    """Read the display label a node definition declares for one input, if it declares one."""

    inputs = definition.get("input") if isinstance(definition, Mapping) else None
    if not isinstance(inputs, Mapping):
        return None
    for group in ("required", "optional"):
        declared = inputs.get(group)
        entry = declared.get(input_key) if isinstance(declared, Mapping) else None
        options = (
            entry[1]
            if isinstance(entry, Sequence)
            and not isinstance(entry, (str, bytes))
            and len(entry) > 1
            else None
        )
        if isinstance(options, Mapping):
            for name in ("label", "localized_name"):
                label = options.get(name)
                if isinstance(label, str) and label.strip():
                    return label.strip()
    return None


__all__ = ["WorkflowScriptProjectionError", "project_workflow_script"]
