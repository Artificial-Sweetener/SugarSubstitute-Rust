#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Prove workflow-level value relations preserve ordinary Comfy topology."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
from typing import cast

from sugarcubes.workflow import COMPOSITION_METADATA_KEY, ComposedValueMaterializer
from sugarcubes.cube_model.native_subgraph_defaults import native_subgraph_defaults
from tests.execution.support.execution_fixtures import cube_document, cube_workflow


def test_field_link_copies_values_without_creating_cross_cube_edges() -> None:
    """Materialize one explicit relation while leaving root links byte-equivalent."""

    workflow = _two_prompt_workflow("anchor", "local")
    original_links = deepcopy(workflow["links"])
    _set_relations(
        workflow,
        [
            {
                "relation_id": "prompt-link",
                "kind": "field_link",
                "source": _endpoint("first", "prompt", "text"),
                "targets": [_endpoint("second", "prompt", "text")],
            }
        ],
    )

    materialized = ComposedValueMaterializer().materialize(workflow)

    assert _prompt_value(materialized, "first") == "anchor"
    assert _prompt_value(materialized, "second") == "anchor"
    assert materialized["links"] == original_links
    assert workflow["links"] == original_links
    assert _prompt_value(workflow, "second") == "local"


def test_field_link_uses_instance_inputs_without_rewriting_flavor_presets() -> None:
    """Keep presets independent from both source and target execution values."""

    workflow = _two_prompt_workflow("instance source", "instance target")
    first = _document(workflow, "first")
    second = _document(workflow, "second")
    _default_flavor_values(first)["prompt.text"] = "stale source preset"
    _default_flavor_values(second)["prompt.text"] = "target preset"
    _set_relations(
        workflow,
        [
            {
                "relation_id": "prompt-link",
                "kind": "field_link",
                "source": _endpoint("first", "prompt", "text"),
                "targets": [_endpoint("second", "prompt", "text")],
            }
        ],
    )

    materialized = ComposedValueMaterializer().materialize(workflow)

    assert _prompt_value(materialized, "second") == "instance source"
    assert _default_flavor_values(_document(materialized, "first")) == {
        "prompt.text": "stale source preset"
    }
    assert _default_flavor_values(_document(materialized, "second")) == {
        "prompt.text": "target preset"
    }


def test_equal_unlinked_fields_remain_independent() -> None:
    """Never infer a relation merely because two authored values are equal."""

    workflow = _two_prompt_workflow("same", "same")

    materialized = ComposedValueMaterializer().materialize(workflow)

    assert materialized == workflow


def test_global_override_projects_one_literal_to_explicit_participants() -> None:
    """Apply an override only to its stable declared participant fields."""

    workflow = _two_prompt_workflow("first", "second")
    _set_relations(
        workflow,
        [
            {
                "relation_id": "global-prompt",
                "kind": "global_override",
                "value": "shared",
                "targets": [
                    _endpoint("first", "prompt", "text"),
                    _endpoint("second", "prompt", "text"),
                ],
            }
        ],
    )

    materialized = ComposedValueMaterializer().materialize(workflow)

    assert _prompt_value(materialized, "first") == "shared"
    assert _prompt_value(materialized, "second") == "shared"


def test_distinct_override_values_produce_distinct_graph_inputs() -> None:
    """Keep changed seeds out of Comfy cache identity while identical seeds match."""

    first = _seed_workflow(101)
    second = _seed_workflow(202)
    repeated = _seed_workflow(101)

    first_materialized = ComposedValueMaterializer().materialize(first)
    second_materialized = ComposedValueMaterializer().materialize(second)
    repeated_materialized = ComposedValueMaterializer().materialize(repeated)

    assert _seed_values(first_materialized) == (101, 101)
    assert _seed_values(second_materialized) == (202, 202)
    assert first_materialized != second_materialized
    assert first_materialized == repeated_materialized


def test_global_override_projects_into_nested_subgraph_boundary_widget() -> None:
    """Resolve one stable field through a native subgraph wrapper boundary."""

    workflow = cube_workflow(
        {
            "nested": _nested_sampler_document("Nested"),
            "direct": _sampler_document("Direct"),
        }
    )
    _set_relations(
        workflow,
        [
            {
                "relation_id": "seed",
                "kind": "global_override",
                "value": 101,
                "targets": [
                    _endpoint("nested", "sampler", "seed"),
                    _endpoint("direct", "sampler", "seed"),
                ],
            }
        ],
    )

    materialized = ComposedValueMaterializer().materialize(workflow)

    nested = _document(materialized, "nested")
    implementation = cast(Mapping[str, object], nested["implementation"])
    subgraphs = cast(list[Mapping[str, object]], implementation["subgraphs"])
    definitions = cast(Mapping[str, object], implementation["definitions"])
    assert native_subgraph_defaults(subgraphs[0], definitions)["seed"] == 101
    assert _node_input(materialized, "direct", "sampler", "seed") == 101


def test_stale_relation_is_inert_instead_of_rejecting_workflow() -> None:
    """Ignore stale annotations without blocking otherwise valid execution."""

    workflow = _two_prompt_workflow("first", "second")
    _set_relations(
        workflow,
        [
            {
                "relation_id": "stale",
                "kind": "field_link",
                "source": _endpoint("missing", "prompt", "text"),
                "targets": [_endpoint("second", "prompt", "text")],
            }
        ],
    )

    materialized = ComposedValueMaterializer().materialize(workflow)

    assert _prompt_value(materialized, "first") == "first"
    assert _prompt_value(materialized, "second") == "second"


def _two_prompt_workflow(first: str, second: str) -> dict[str, object]:
    """Build two independently authored prompt Cubes."""

    return cube_workflow(
        {
            "first": _prompt_document("First", first),
            "second": _prompt_document("Second", second),
        }
    )


def _prompt_document(alias: str, value: str) -> dict[str, object]:
    """Build one document with a stable prompt field identity."""

    return cube_document(
        alias,
        nodes={"prompt": {"class_type": "Prompt", "inputs": {"text": value}}},
        definitions={
            "Prompt": {
                "input": {"required": {"text": ["STRING", {"default": ""}]}},
                "output": ["CONDITIONING"],
            },
            "TopologyRelay": {
                "input": {"required": {"resource": ["STRING"]}},
                "output": ["STRING"],
            },
        },
    )


def _seed_workflow(value: int) -> dict[str, object]:
    """Build two sampler Cubes with one explicit shared seed relation."""

    workflow = cube_workflow(
        {
            "first": _sampler_document("First"),
            "second": _sampler_document("Second"),
        }
    )
    _set_relations(
        workflow,
        [
            {
                "relation_id": "seed",
                "kind": "global_override",
                "value": value,
                "targets": [
                    _endpoint("first", "sampler", "seed"),
                    _endpoint("second", "sampler", "seed"),
                ],
            }
        ],
    )
    return workflow


def _sampler_document(alias: str) -> dict[str, object]:
    """Build one Cube with a stable non-surface sampler seed field."""

    return cube_document(
        alias,
        nodes={"sampler": {"class_type": "Sampler", "inputs": {"seed": 0}}},
        definitions={
            "Sampler": {
                "input": {"required": {"seed": ["INT", {"default": 0}]}},
                "output": ["LATENT"],
            }
        },
    )


def _nested_sampler_document(alias: str) -> dict[str, object]:
    """Build one sampler whose stable fields are native wrapper inputs."""

    sampler_definition = {
        "input": {
            "required": {
                "seed": ["INT", {"default": 0, "control_after_generate": True}],
                "steps": ["INT", {"default": 20}],
            }
        },
        "input_order": {"required": ["seed", "steps"]},
        "output": ["LATENT"],
    }
    subgraph = {
        "id": "sampler-wrapper",
        "inputs": [
            {"name": "seed", "type": "INT", "linkIds": [1]},
            {"name": "steps", "type": "INT", "linkIds": [2]},
        ],
        "nodes": [
            {
                "id": 7,
                "type": "Sampler",
                "inputs": [
                    {"name": "seed", "link": 1, "widget": {"name": "seed"}},
                    {"name": "steps", "link": 2, "widget": {"name": "steps"}},
                ],
                "widgets_values": [7, "randomize", 12],
            }
        ],
        "links": [
            {
                "id": 1,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 7,
                "target_slot": 0,
            },
            {
                "id": 2,
                "origin_id": -10,
                "origin_slot": 1,
                "target_id": 7,
                "target_slot": 1,
            },
        ],
    }
    return cube_document(
        alias,
        nodes={"sampler": {"class_type": "sampler-wrapper", "inputs": {}}},
        definitions={"Sampler": sampler_definition},
        subgraphs=[subgraph],
    )


def _seed_values(workflow: Mapping[str, object]) -> tuple[object, object]:
    """Return both stable sampler seed values from one materialized graph."""

    return (
        _node_input(workflow, "first", "sampler", "seed"),
        _node_input(workflow, "second", "sampler", "seed"),
    )


def _set_relations(
    workflow: dict[str, object], relations: list[dict[str, object]]
) -> None:
    """Attach schema-one composition metadata to a fixture workflow."""

    extra = cast(dict[str, object], workflow["extra"])
    extra[COMPOSITION_METADATA_KEY] = {
        "schema_version": 1,
        "value_relations": relations,
    }


def _endpoint(instance_id: str, node_symbol: str, input_name: str) -> dict[str, str]:
    """Build one stable field endpoint payload."""

    return {
        "instance_id": instance_id,
        "node_symbol": node_symbol,
        "input_name": input_name,
    }


def _prompt_value(workflow: Mapping[str, object], instance_id: str) -> object:
    """Read one raw embedded prompt value from a fixture graph."""

    return _node_input(workflow, instance_id, "prompt", "text")


def _document(workflow: Mapping[str, object], instance_id: str) -> dict[str, object]:
    """Return one mutable embedded document by stable instance identity."""

    definitions = cast(Mapping[str, object], workflow["definitions"])
    subgraphs = cast(list[dict[str, object]], definitions["subgraphs"])
    definition = next(
        item for item in subgraphs if item.get("id") == f"definition-{instance_id}"
    )
    extra = cast(dict[str, object], definition["extra"])
    return cast(dict[str, object], extra["sugarcubes_document"])


def _default_flavor_values(document: Mapping[str, object]) -> dict[str, object]:
    """Return one fixture document's mutable default-preset values."""

    flavors = cast(Mapping[str, object], document["flavors"])
    authored = cast(list[Mapping[str, object]], flavors["authored"])
    return cast(dict[str, object], authored[0]["values"])


def _node_input(
    workflow: Mapping[str, object],
    instance_id: str,
    node_symbol: str,
    input_name: str,
) -> object:
    """Read one raw embedded stable field value from a fixture graph."""

    definitions = cast(Mapping[str, object], workflow["definitions"])
    subgraphs = cast(list[Mapping[str, object]], definitions["subgraphs"])
    definition = next(
        item for item in subgraphs if item.get("id") == f"definition-{instance_id}"
    )
    extra = cast(Mapping[str, object], definition["extra"])
    document = cast(Mapping[str, object], extra["sugarcubes_document"])
    implementation = cast(Mapping[str, object], document["implementation"])
    nodes = cast(Mapping[str, Mapping[str, object]], implementation["nodes"])
    inputs = cast(Mapping[str, object], nodes[node_symbol]["inputs"])
    return inputs[input_name]
