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
from dataclasses import replace

from ..cube_model import CubeDocument
from ..execution import ConnectionOrigin
from ..language.compiler_models import MODEL_ASSET_NAMESPACE, PROMPT_LORA_NAMESPACE
from ..language.substitute_writer import (
    ScriptConnection,
    ScriptCube,
    ScriptField,
    ScriptFieldReference,
    ScriptNode,
    ScriptOverride,
    SubstituteScript,
)
from ..workflow_analysis import CubeGraphAnalysis
from .cube_input_declarations import CubeInputDeclarations
from .workflow_script_state import WorkflowScriptState, read_workflow_script_state

_DOCUMENT_KEY = "sugarcubes_document"
_COMPOSITION_KEY = "sugarcubes_composition"
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
    hashes = _field_hashes(analysis.normalized_workflow)
    state = read_workflow_script_state(analysis.normalized_workflow)
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
    driven: dict[str, object] = {}
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
                nodes=_script_nodes(
                    document,
                    hashes.get(instance.instance_id, {}),
                    instance.instance_id,
                    state,
                    driven,
                ),
                bypassed=instance.execution_mode == _BYPASS_MODE,
                save_outputs=not (
                    isinstance(properties, Mapping)
                    and properties.get(_SAVE_OUTPUTS_KEY) is False
                ),
            )
        )
    alias_of = {instance.instance_id: instance.instance_alias for instance in ordered}
    instance_ids = list(alias_of)
    return SubstituteScript(
        cubes=_with_node_links(cubes, instance_ids, state),
        overrides=(
            *(
                ScriptOverride(
                    key=override.key,
                    label=override.label,
                    value=driven[override.key],
                )
                for override in state.overrides
                if override.key in driven
            ),
            *(
                ScriptOverride(key=key, label=key, value=value, full=False, mode=mode)
                for key, mode, value in state.partial_overrides
                if key not in driven
            ),
        ),
        override_selections=state.override_selections,
        fixed_seed_fields=_fixed_seed_fields(state, cubes, instance_ids),
        fixed_seed_overrides=tuple(
            key for key in state.fixed_seed_overrides if key in driven
        ),
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


_FieldHashes = dict[tuple[str, str], tuple[str | None, tuple[tuple[str, str], ...]]]


def _field_hashes(workflow: Mapping[str, object]) -> dict[str, _FieldHashes]:
    """Read the model and LoRA hashes a workflow keeps for its fields, by Cube instance."""

    extra = workflow.get("extra")
    composition = extra.get(_COMPOSITION_KEY) if isinstance(extra, Mapping) else None
    annotations = (
        composition.get("field_annotations")
        if isinstance(composition, Mapping)
        else None
    )
    hashes: dict[str, _FieldHashes] = {}
    if not isinstance(annotations, Sequence) or isinstance(annotations, (str, bytes)):
        return hashes
    for annotation in annotations:
        if not isinstance(annotation, Mapping):
            continue
        endpoint, payload = annotation.get("endpoint"), annotation.get("payload")
        if not isinstance(endpoint, Mapping) or not isinstance(payload, Mapping):
            continue
        instance = endpoint.get("instance_id")
        field = (endpoint.get("node_symbol"), endpoint.get("input_name"))
        if not isinstance(instance, str) or not all(
            isinstance(part, str) for part in field
        ):
            continue
        key = (str(field[0]), str(field[1]))
        model, loras = hashes.setdefault(instance, {}).get(key, (None, ()))
        if annotation.get("namespace") == MODEL_ASSET_NAMESPACE:
            sha256 = payload.get("sha256")
            model = sha256 if isinstance(sha256, str) else model
        elif annotation.get("namespace") == PROMPT_LORA_NAMESPACE:
            listed = payload.get("loras")
            if isinstance(listed, Sequence) and not isinstance(listed, (str, bytes)):
                loras = tuple(
                    (str(lora["name"]), str(lora["sha256"]))
                    for lora in listed
                    if isinstance(lora, Mapping)
                    and isinstance(lora.get("name"), str)
                    and isinstance(lora.get("sha256"), str)
                )
        hashes[instance][key] = (model, loras)
    return hashes


def _with_node_links(
    cubes: Sequence[ScriptCube],
    instance_ids: Sequence[str],
    state: WorkflowScriptState,
) -> tuple[ScriptCube, ...]:
    """Name, on each node that follows another Cube's node, the node it follows."""

    named = {
        (instance_id, node.key): (cube.alias, node.label)
        for instance_id, cube in zip(instance_ids, cubes, strict=True)
        for node in cube.nodes
    }
    return tuple(
        replace(
            cube,
            nodes=tuple(
                replace(
                    node,
                    linked_to=named.get(
                        state.node_links.get((instance_id, node.key), ("", ""))
                    ),
                )
                for node in cube.nodes
            ),
        )
        for instance_id, cube in zip(instance_ids, cubes, strict=True)
    )


def _fixed_seed_fields(
    state: WorkflowScriptState,
    cubes: Sequence[ScriptCube],
    instance_ids: Sequence[str],
) -> tuple[ScriptFieldReference, ...]:
    """Name the fields whose seed stays fixed, where the workflow still holds them."""

    named = {
        (instance_id, node.key, field.input_key): ScriptFieldReference(
            cube.alias, node.label, field.input_label
        )
        for instance_id, cube in zip(instance_ids, cubes, strict=True)
        for node in cube.nodes
        for field in node.fields
    }
    return tuple(
        named[identity] for identity in state.fixed_seed_fields if identity in named
    )


def _script_nodes(
    document: CubeDocument,
    hashes: _FieldHashes,
    instance_id: str,
    state: WorkflowScriptState,
    driven: dict[str, object],
) -> tuple[ScriptNode, ...]:
    """Select and name the fields one Cube document exposes.

    Nodes are ordered by their key and a node's fields by their input name, as the persisted
    format orders them. A field a workflow-wide override drives is marked as such, and the
    value of the first such field of each override is recorded in `driven`.
    """

    declarations = CubeInputDeclarations(document)
    nodes: list[ScriptNode] = []
    for key, node in sorted(document.implementation.nodes.items()):
        inputs = node.get("inputs")
        fields: list[ScriptField] = []
        for input_key, held in sorted(
            inputs.items() if isinstance(inputs, Mapping) else ()
        ):
            identity = (instance_id, key, input_key)
            if not isinstance(input_key, str) or not (
                (key, input_key) in declarations.exposed
                or identity in state.authored_fields
            ):
                continue
            value = declarations.typed(node, input_key, held)
            model_sha256, lora_sha256 = hashes.get((key, input_key), (None, ()))
            fields.append(
                ScriptField(
                    input_key=input_key,
                    input_label=declarations.label(key, node, input_key),
                    # A node that follows another keeps its own value in the script.
                    value=(
                        declarations.typed(
                            node, input_key, state.own_values.get(identity, held)
                        )
                        if (instance_id, key) in state.node_links
                        else value
                    ),
                    model_sha256=model_sha256,
                    lora_sha256=lora_sha256,
                    owned_by_override=_driven_by(state, identity, value, driven),
                )
            )
        label = node.get("label")
        mode = node.get("mode")
        enabled = node.get("enabled")
        nodes.append(
            ScriptNode(
                key=key,
                label=(
                    label.strip() if isinstance(label, str) and label.strip() else key
                ),
                fields=tuple(fields),
                mode=(
                    mode if isinstance(mode, int) and not isinstance(mode, bool) else 0
                ),
                enabled=enabled if isinstance(enabled, bool) else None,
                revealed=node.get("revealed") is True,
            )
        )
    return tuple(nodes)


def _driven_by(
    state: WorkflowScriptState,
    identity: tuple[str, str, str],
    value: object,
    driven: dict[str, object],
) -> bool:
    """Report whether an override drives one field, keeping the first value seen for it.

    Every field an override drives holds its value, and a field keeps a whole number exactly
    where the page's own record of the override may have rounded it.
    """

    for override in state.overrides:
        if override.owns(identity):
            driven.setdefault(override.key, value)
            return True
    return False


__all__ = ["WorkflowScriptProjectionError", "project_workflow_script"]
