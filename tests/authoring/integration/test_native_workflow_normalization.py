#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Prove exact-version normalization of saved native Cube state."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping, cast

from sugarcubes.authoring import NativeWorkflowNormalizer
from sugarcubes.cube_model import CubeDocument
from tests.execution.support.execution_fixtures import (
    cube_document,
    cube_workflow,
    image_passthrough_document,
)


def test_normalizer_attaches_named_values_and_reconciled_document() -> None:
    """Recover positional state without requiring Comfy frontend enrichment."""

    document = cube_document(
        "Widget",
        nodes={"widget": {"class_type": "WidgetNode", "inputs": {"value": 3}}},
        definitions={
            "WidgetNode": {
                "input": {"required": {"value": ["INT", {"default": 3}]}},
                "output": ["INT"],
            }
        },
    )
    document["surface"] = {
        "default_flavor_id": "default",
        "controls": [
            {
                "control_id": "widget.value",
                "symbol": "widget",
                "input_name": "value",
                "label": "value",
                "class_type": "WidgetNode",
                "value_type": "number",
            }
        ],
    }
    workflow = cube_workflow({"instance": document})
    definitions = cast(Mapping[str, object], workflow["definitions"])
    subgraphs = cast(list[dict[str, object]], definitions["subgraphs"])
    definition = subgraphs[0]
    extra = cast(dict[str, object], definition["extra"])
    extra.pop("sugarcubes_document")
    definition["nodes"] = [
        {
            "id": "saved-widget",
            "type": "WidgetNode",
            "widgets_values": [7],
            "properties": {"sugarcubes_symbol": "widget"},
        }
    ]

    normalized = NativeWorkflowNormalizer(_Resolver(document)).normalize(workflow)

    normalized_definitions = cast(Mapping[str, object], normalized["definitions"])
    normalized_subgraphs = cast(
        list[dict[str, object]], normalized_definitions["subgraphs"]
    )
    normalized_definition = normalized_subgraphs[0]
    saved_node = cast(list[dict[str, object]], normalized_definition["nodes"])[0]
    assert saved_node["sugarcubes_widget_values"] == {"value": 7}
    normalized_extra = cast(Mapping[str, object], normalized_definition["extra"])
    reconciled = cast(Mapping[str, object], normalized_extra["sugarcubes_document"])
    implementation = cast(Mapping[str, object], reconciled["implementation"])
    nodes = cast(Mapping[str, Mapping[str, object]], implementation["nodes"])
    assert nodes["widget"]["inputs"] == {"value": 7}


def test_normalizer_rebases_embedded_cube_on_exact_pinned_definition() -> None:
    """Use embedded state as values, never as the pinned definition authority."""

    exact = cube_document(
        "Widget",
        nodes={"widget": {"class_type": "WidgetNode", "inputs": {"value": 3}}},
        definitions={
            "WidgetNode": {
                "input": {"required": {"value": ["INT", {"default": 3}]}},
                "output": ["INT"],
            }
        },
    )
    exact["metadata"] = {
        "default_alias": "Anima/Widget",
        "target_model": "Anima",
    }
    stale = deepcopy(exact)
    stale["metadata"] = {"default_alias": "Anima/Widget"}
    workflow = cube_workflow({"instance": stale})

    normalized = NativeWorkflowNormalizer(_Resolver(exact)).normalize(workflow)

    normalized_definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], normalized["definitions"])["subgraphs"],
    )[0]
    normalized_extra = cast(Mapping[str, object], normalized_definition["extra"])
    reconciled = cast(Mapping[str, object], normalized_extra["sugarcubes_document"])
    assert reconciled["metadata"] == {
        "default_alias": "Anima/Widget",
        "target_model": "Anima",
    }


def test_normalizer_rebases_structure_without_discarding_embedded_instance_values() -> (
    None
):
    """Preserve authored instance data through exact-definition normalization."""

    exact = cube_document(
        "Widget",
        nodes={"widget": {"class_type": "WidgetNode", "inputs": {"value": 3}}},
        definitions={
            "WidgetNode": {
                "input": {"required": {"value": ["INT", {"default": 3}]}},
                "output": ["INT"],
            }
        },
    )
    exact["surface"] = {
        "default_flavor_id": "default",
        "controls": [
            {
                "control_id": "widget.value",
                "symbol": "widget",
                "input_name": "value",
                "label": "Value",
                "class_type": "WidgetNode",
                "value_type": "number",
            }
        ],
    }
    exact["flavors"] = {
        "authored": [
            {"id": "default", "name": "Default", "values": {"widget.value": 3}}
        ]
    }
    embedded = deepcopy(exact)
    embedded_nodes = cast(
        dict[str, dict[str, object]],
        cast(dict[str, object], embedded["implementation"])["nodes"],
    )
    cast(dict[str, object], embedded_nodes["widget"]["inputs"])["value"] = 17
    embedded_nodes["widget"]["substitute_relation"] = {"source": "Earlier.widget"}
    workflow = cube_workflow({"instance": embedded})

    normalized = NativeWorkflowNormalizer(_Resolver(exact)).normalize(workflow)

    normalized_definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], normalized["definitions"])["subgraphs"],
    )[0]
    normalized_extra = cast(Mapping[str, object], normalized_definition["extra"])
    reconciled = cast(dict[str, object], normalized_extra["sugarcubes_document"])
    implementation = cast(dict[str, object], reconciled["implementation"])
    nodes = cast(dict[str, dict[str, object]], implementation["nodes"])
    inputs = cast(dict[str, object], nodes["widget"]["inputs"])
    assert inputs["value"] == 17
    assert nodes["widget"]["substitute_relation"] == {"source": "Earlier.widget"}
    flavors = cast(dict[str, object], reconciled["flavors"])
    authored = cast(list[dict[str, object]], flavors["authored"])
    assert cast(dict[str, object], authored[0]["values"])["widget.value"] == 3


def test_normalizer_preserves_non_surface_stable_instance_fields() -> None:
    """Preserve executable sampler state even when it is not editor-facing."""

    exact = cube_document(
        "Sampler",
        nodes={
            "sampler": {
                "class_type": "SamplerNode",
                "inputs": {"seed": 1, "batch_size": 1, "steps": 20},
            }
        },
        definitions={
            "SamplerNode": {
                "input": {
                    "required": {
                        "seed": ["INT", {"default": 1}],
                        "batch_size": ["INT", {"default": 1}],
                        "steps": ["INT", {"default": 20}],
                    }
                },
                "input_order": {
                    "required": ["seed", "batch_size", "steps"],
                },
                "output": ["IMAGE"],
            }
        },
    )
    exact["surface"] = {
        "default_flavor_id": "default",
        "controls": [
            {
                "control_id": "sampler.steps",
                "symbol": "sampler",
                "input_name": "steps",
                "label": "Steps",
                "class_type": "SamplerNode",
                "value_type": "number",
            }
        ],
    }
    exact["flavors"] = {
        "authored": [
            {"id": "default", "name": "Default", "values": {"sampler.steps": 20}}
        ]
    }
    embedded = deepcopy(exact)
    embedded_nodes = cast(
        dict[str, dict[str, object]],
        cast(dict[str, object], embedded["implementation"])["nodes"],
    )
    embedded_inputs = cast(dict[str, object], embedded_nodes["sampler"]["inputs"])
    embedded_inputs.update({"seed": 987654321, "batch_size": 2, "steps": 28})
    embedded_flavors = cast(dict[str, object], embedded["flavors"])
    embedded_authored = cast(list[dict[str, object]], embedded_flavors["authored"])
    cast(dict[str, object], embedded_authored[0]["values"])["sampler.steps"] = 28
    workflow = cube_workflow({"instance": embedded})

    normalized = NativeWorkflowNormalizer(_Resolver(exact)).normalize(workflow)

    normalized_definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], normalized["definitions"])["subgraphs"],
    )[0]
    normalized_extra = cast(Mapping[str, object], normalized_definition["extra"])
    reconciled = cast(Mapping[str, object], normalized_extra["sugarcubes_document"])
    implementation = cast(Mapping[str, object], reconciled["implementation"])
    nodes = cast(Mapping[str, Mapping[str, object]], implementation["nodes"])
    assert nodes["sampler"]["inputs"] == {
        "seed": 987654321,
        "batch_size": 2,
        "steps": 28,
    }


def test_normalizer_preserves_nested_wrapper_widget_values_from_embedded_graph() -> (
    None
):
    """Keep canonical wrapper values instead of restoring nested defaults."""

    nested_definition = {
        "id": "nested-definition",
        "inputs": [
            {
                "id": "boundary-value",
                "label": "batch_size",
                "name": "batch_size",
                "type": "INT",
                "linkIds": [10],
            }
        ],
        "outputs": [],
        "nodes": [
            {
                "id": 3,
                "type": "WidgetNode",
                "inputs": [
                    {
                        "name": "batch_size",
                        "type": "INT",
                        "link": 10,
                        "widget": {"name": "batch_size"},
                    }
                ],
                "widgets_values": [1],
            }
        ],
        "links": [
            {
                "id": 10,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 3,
                "target_slot": 0,
                "type": "INT",
            }
        ],
    }
    exact = cube_document(
        "Nested",
        nodes={
            "sampler": {
                "class_type": "nested-definition",
                "inputs": {"batch_size": 1},
            }
        },
        definitions={
            "WidgetNode": {
                "input": {"required": {"batch_size": ["INT", {"default": 1}]}},
                "input_order": {"required": ["batch_size"]},
                "output": ["LATENT"],
            }
        },
        subgraphs=(nested_definition,),
    )
    embedded = deepcopy(exact)
    embedded_nodes = cast(
        dict[str, dict[str, object]],
        cast(dict[str, object], embedded["implementation"])["nodes"],
    )
    cast(dict[str, object], embedded_nodes["sampler"]["inputs"])["batch_size"] = 2

    normalized = NativeWorkflowNormalizer(_Resolver(exact)).normalize(
        cube_workflow({"instance": embedded})
    )

    normalized_definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], normalized["definitions"])["subgraphs"],
    )[0]
    normalized_extra = cast(Mapping[str, object], normalized_definition["extra"])
    reconciled = cast(Mapping[str, object], normalized_extra["sugarcubes_document"])
    implementation = cast(Mapping[str, object], reconciled["implementation"])
    nodes = cast(Mapping[str, Mapping[str, object]], implementation["nodes"])
    assert nodes["sampler"]["inputs"] == {"batch_size": 2}


def test_normalizer_preserves_ordinary_graph_content_byte_for_byte_semantically() -> (
    None
):
    """Leave non-Cube nodes and metadata entirely outside Cube reconciliation."""

    ordinary_workflow: dict[str, object] = {
        "last_node_id": 9,
        "last_link_id": 4,
        "nodes": [
            {
                "id": 9,
                "type": "ThirdPartyNode",
                "pos": [13.25, -4.5],
                "inputs": [{"name": "renamed_later", "type": "CUSTOM", "link": 4}],
                "widgets_values": [{"host_owned": [1, 2, 3]}],
                "properties": {"extension_private": {"future": True}},
            }
        ],
        "links": [[4, 8, 0, 9, 0, "CUSTOM"]],
        "groups": [{"title": "ordinary", "bounding": [0, 0, 10, 10]}],
        "extra": {"third_party": {"unknown": "preserve"}},
        "version": 0.4,
    }
    before = deepcopy(ordinary_workflow)

    normalized = NativeWorkflowNormalizer(_UnexpectedResolver()).normalize(
        ordinary_workflow
    )

    assert normalized == before
    assert ordinary_workflow == before


def test_normalizer_restores_exact_cube_sockets_without_touching_ordinary_nodes() -> (
    None
):
    """Use pinned Cube boundaries while leaving adjacent native nodes opaque."""

    document = image_passthrough_document("Image")
    workflow = cube_workflow({"cube-a": document})
    cube_node = cast(list[dict[str, object]], workflow["nodes"])[0]
    cube_node["inputs"] = [{"name": "stale-input", "type": "MASK"}]
    cube_node["outputs"] = [{"name": "stale-output", "type": "MASK"}]
    ordinary = {
        "id": 99,
        "type": "ThirdParty.Custom",
        "inputs": [{"name": "vendor", "type": "VENDOR_TYPE"}],
        "outputs": [{"name": "opaque", "type": "OPAQUE_TYPE"}],
        "properties": {"vendor": {"keep": True}},
    }
    cast(list[dict[str, object]], workflow["nodes"]).append(ordinary)

    normalized = NativeWorkflowNormalizer(_Resolver(document)).normalize(workflow)

    normalized_nodes = cast(list[dict[str, object]], normalized["nodes"])
    assert normalized_nodes[0]["inputs"] == [{"name": "input.image", "type": "IMAGE"}]
    assert normalized_nodes[0]["outputs"] == [{"name": "output.image", "type": "IMAGE"}]
    assert normalized_nodes[1] == ordinary


def test_normalizer_uses_stable_surface_fields_and_discards_host_only_widgets() -> None:
    """Decode executable values without mistaking trailing UI state for fields."""

    document = cube_document(
        "Masks",
        nodes={
            "masks": {
                "class_type": "MaskBatch",
                "inputs": {"image": [], "channel": "red"},
            }
        },
        definitions={
            "MaskBatch": {
                "input": {"required": {"image": ["LIST"], "channel": ["LIST"]}},
                "input_order": {"required": ["image", "channel"]},
                "output": ["MASK"],
            }
        },
    )
    document["surface"] = {
        "default_flavor_id": "default",
        "controls": [
            {
                "control_id": f"masks.{name}",
                "symbol": "masks",
                "input_name": name,
                "label": name,
                "class_type": "MaskBatch",
                "value_type": value_type,
            }
            for name, value_type in (("image", "object"), ("channel", "string"))
        ],
    }
    workflow = cube_workflow({"instance": document})
    definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], workflow["definitions"])["subgraphs"],
    )[0]
    cast(dict[str, object], definition["extra"]).pop("sugarcubes_document")
    definition["nodes"] = [
        {
            "id": "saved-masks",
            "type": "MaskBatch",
            "inputs": [],
            "widgets_values": [
                ["mask-a.png", "mask-b.png"],
                "green",
                "ui-preview-kind",
                "ui-order-mode",
            ],
            "properties": {"sugarcubes_symbol": "masks"},
        }
    ]

    normalized = NativeWorkflowNormalizer(_Resolver(document)).normalize(workflow)

    normalized_definition = cast(
        list[dict[str, object]],
        cast(Mapping[str, object], normalized["definitions"])["subgraphs"],
    )[0]
    saved_node = cast(list[dict[str, object]], normalized_definition["nodes"])[0]
    assert saved_node["sugarcubes_widget_values"] == {
        "image": ["mask-a.png", "mask-b.png"],
        "channel": "green",
    }


class _Resolver:
    """Return one exact fixture document."""

    def __init__(self, document: Mapping[str, object]) -> None:
        """Capture the exact artifact payload."""

        self._document = document

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return the exact requested document."""

        document = CubeDocument.from_dict(self._document)
        assert cube_id == document.cube_id
        assert version_pin == document.version
        return document


class _UnexpectedResolver:
    """Fail if ordinary graph normalization attempts Cube interpretation."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Reject an out-of-scope catalog lookup."""

        raise AssertionError((cube_id, version_pin))
