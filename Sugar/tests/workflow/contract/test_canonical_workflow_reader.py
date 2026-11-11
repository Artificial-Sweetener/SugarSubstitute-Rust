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
"""Prove canonical native workflow validation and embedded authority."""

from __future__ import annotations

from copy import deepcopy

import pytest

from sugarcubes.cube_model import CubeDocument
from sugarcubes.workflow import CanonicalWorkflowError, read_canonical_workflow
from sugarcubes.workflow.limits import MAX_WORKFLOW_DEFINITIONS, MAX_WORKFLOW_NODES
from tests.workflow.support.workflow_fixtures import cube_document, cube_workflow


def test_reads_embedded_cube_without_a_catalog_and_preserves_loose_nodes() -> None:
    """Treat a valid embedded definition as executable workflow truth."""

    source = cube_workflow()

    workflow = read_canonical_workflow(source)

    assert [(item.instance_id, item.node_id) for item in workflow.instances] == [
        ("cube-sdxl-text-1", "7")
    ]
    assert workflow.definitions[0].cube_id.endswith("SDXL/Text to Image.cube")
    assert len(workflow.definitions[0].semantic_hash) == 64
    assert workflow.payload == source
    assert workflow.payload is not source
    nodes = workflow.payload["nodes"]
    assert isinstance(nodes, list)
    assert len(nodes) == 2


def test_preserves_valid_native_cube_execution_mode() -> None:
    """Keep root bypass and disable state in the presentation-neutral model."""

    source = cube_workflow()
    nodes = source["nodes"]
    assert isinstance(nodes, list)
    node = nodes[0]
    assert isinstance(node, dict)
    node["mode"] = 4

    workflow = read_canonical_workflow(source)

    assert workflow.instances[0].execution_mode == 4


@pytest.mark.parametrize("mode", [True, -1, 5, "4"])
def test_rejects_invalid_native_cube_execution_mode(mode: object) -> None:
    """Reject malformed host modes instead of silently changing execution."""

    source = cube_workflow()
    nodes = source["nodes"]
    assert isinstance(nodes, list)
    node = nodes[0]
    assert isinstance(node, dict)
    node["mode"] = mode

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.invalid_cube_execution_mode"


def test_hashes_are_deterministic_and_change_with_embedded_behavior() -> None:
    """Ignore mapping insertion order while detecting canonical document drift."""

    first = cube_workflow()
    reordered = {key: first[key] for key in reversed(first)}
    changed = deepcopy(first)
    definitions = changed["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    definition = subgraphs[0]
    assert isinstance(definition, dict)
    extra = definition["extra"]
    assert isinstance(extra, dict)
    document = extra["sugarcubes_document"]
    assert isinstance(document, dict)
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    implementation["nodes"] = {
        "checkpoint": {"class_type": "CheckpointLoaderSimple", "inputs": {}}
    }

    assert (
        read_canonical_workflow(first).semantic_hash
        == read_canonical_workflow(reordered).semantic_hash
    )
    assert read_canonical_workflow(first).definitions[0].semantic_hash != (
        read_canonical_workflow(changed).definitions[0].semantic_hash
    )


def test_validates_embedded_document_identity_and_retains_canonical_content() -> None:
    """Use the portable Cube document as catalog-comparable semantic content."""

    workflow = read_canonical_workflow(cube_workflow())
    definition = workflow.definitions[0]

    assert definition.document == CubeDocument.from_dict(cube_document()).to_dict()
    assert (
        definition.semantic_hash
        == read_canonical_workflow(cube_workflow()).definitions[0].semantic_hash
    )


def test_ignores_stale_embedded_document_identity_divergence() -> None:
    """Let authoritative native state run when its auxiliary document is stale."""

    source = cube_workflow()
    definitions = source["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    definition = subgraphs[0]
    assert isinstance(definition, dict)
    extra = definition["extra"]
    assert isinstance(extra, dict)
    document = extra["sugarcubes_document"]
    assert isinstance(document, dict)
    document["version"] = "9.0.0"

    embedded = read_canonical_workflow(source).definitions[0]

    assert embedded.document is None
    assert len(embedded.semantic_hash) == 64


def test_legacy_embedded_definition_without_document_remains_runnable() -> None:
    """Retain native state for workflows saved before source embedding."""

    source = cube_workflow()
    definitions = source["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    definition = subgraphs[0]
    assert isinstance(definition, dict)
    extra = definition["extra"]
    assert isinstance(extra, dict)
    extra.pop("sugarcubes_document")

    embedded = read_canonical_workflow(source).definitions[0]

    assert embedded.document is None
    assert len(embedded.semantic_hash) == 64


def test_rejects_marked_instances_without_their_embedded_definition() -> None:
    """Fail closed instead of consulting an installed catalog for missing content."""

    source = cube_workflow()
    source["definitions"] = {"subgraphs": []}

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.missing_cube_definition"
    assert captured.value.path == "$.nodes[0].type"


def test_rejects_duplicate_stable_instance_identity() -> None:
    """Prevent ambiguous ownership when a copied node retains an instance id."""

    source = cube_workflow()
    nodes = source["nodes"]
    assert isinstance(nodes, list)
    nodes.append(deepcopy(nodes[0]))

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.duplicate_instance"


def test_rejects_instance_and_definition_identity_divergence() -> None:
    """Keep the embedded definition authoritative over stale instance metadata."""

    source = cube_workflow()
    nodes = source["nodes"]
    assert isinstance(nodes, list)
    node = nodes[0]
    assert isinstance(node, dict)
    properties = node["properties"]
    assert isinstance(properties, dict)
    identity = properties["sugarcubes_cube"]
    assert isinstance(identity, dict)
    identity["cube_version"] = "9.9.9"

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.cube_identity_mismatch"


def test_rejects_workflows_above_the_root_node_limit() -> None:
    """Bound untrusted root traversal before copying the complete workflow."""

    source = cube_workflow()
    source["nodes"] = [{}] * (MAX_WORKFLOW_NODES + 1)

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.limit_exceeded"
    assert captured.value.path == "$.nodes"


def test_rejects_workflows_above_the_definition_limit() -> None:
    """Bound embedded-definition scanning independently of catalog behavior."""

    source = cube_workflow()
    source["definitions"] = {"subgraphs": [{}] * (MAX_WORKFLOW_DEFINITIONS + 1)}

    with pytest.raises(CanonicalWorkflowError) as captured:
        read_canonical_workflow(source)

    assert captured.value.code == "workflow.limit_exceeded"
    assert captured.value.path == "$.definitions.subgraphs"
