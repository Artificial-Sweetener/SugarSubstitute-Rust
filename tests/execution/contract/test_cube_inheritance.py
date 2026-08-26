#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Specify directed native MODEL, CLIP, and VAE inheritance."""

from __future__ import annotations

from typing import Mapping

import pytest

from sugarcubes.execution import (
    CubeBoundaryEndpoint,
    CubeInheritanceError,
    CubeInheritanceResolver,
    InheritanceResult,
    NativeCubeWorkflowLowerer,
    ProximityConnection,
    build_cube_topology,
)
from sugarcubes.workflow import read_canonical_workflow
from tests.execution.support.execution_fixtures import (
    consumer_document,
    cube_document,
    cube_workflow,
    provider_document,
)


def test_inherits_model_clip_and_vae_from_unique_nearest_upstream_origin() -> None:
    """Preserve provider semantics while replacing script order with topology."""

    result = _resolve(
        {"cube-a": provider_document(), "cube-b": consumer_document()},
        (_edge("cube-a", "cube-b"),),
    )

    assert _inputs(result.prompt["cube-b:model_use"])["model"] == [
        "cube-a:checkpoint",
        0,
    ]
    assert _inputs(result.prompt["cube-b:clip_use"])["clip_in"] == [
        "cube-a:checkpoint",
        1,
    ]
    assert _inputs(result.prompt["cube-b:vae_use"])["vae"] == [
        "cube-a:checkpoint",
        2,
    ]
    assert [
        (item.slot, item.source_instance_id, item.distance)
        for item in result.inherited_bindings
    ] == [
        ("model", "cube-a", 1),
        ("clip", "cube-a", 1),
        ("vae", "cube-a", 1),
    ]


def test_explicit_input_and_active_local_provider_win() -> None:
    """Never replace authored values or a Cube's own provider origin."""

    target = consumer_document()
    implementation = target["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    nodes["local"] = {"class_type": "CheckpointLoader", "inputs": {}}
    nodes["local_use"] = {
        "class_type": "ModelConsumer",
        "inputs": {"model": ["local", 0]},
    }
    nodes["model_use"]["inputs"]["model"] = ["local", 0]
    nodes["clip_use"]["inputs"]["clip_in"] = "authored"

    result = _resolve(
        {"cube-a": provider_document(), "cube-b": target},
        (_edge("cube-a", "cube-b"),),
    )

    assert _inputs(result.prompt["cube-b:model_use"])["model"] == ["cube-b:local", 0]
    assert _inputs(result.prompt["cube-b:clip_use"])["clip_in"] == "authored"
    assert _inputs(result.prompt["cube-b:vae_use"])["vae"] == [
        "cube-b:local",
        2,
    ]


def test_inheritance_is_transitive_but_never_treats_derived_transform_as_origin() -> (
    None
):
    """Walk Cube topology past a transform-only intermediate Cube."""

    middle = cube_document(
        "Middle",
        nodes={
            "transform": {"class_type": "ModelTransform", "inputs": {"model": None}},
            "use": {
                "class_type": "ModelConsumer",
                "inputs": {"model": ["transform", 0]},
            },
        },
    )
    result = _resolve(
        {
            "cube-a": provider_document(),
            "cube-b": middle,
            "cube-c": consumer_document(),
        },
        (_edge("cube-a", "cube-b"), _edge("cube-b", "cube-c")),
    )

    assert _inputs(result.prompt["cube-b:transform"])["model"] == [
        "cube-a:checkpoint",
        0,
    ]
    assert _inputs(result.prompt["cube-c:model_use"])["model"] == [
        "cube-a:checkpoint",
        0,
    ]
    inherited = next(
        item
        for item in result.inherited_bindings
        if item.target_instance_id == "cube-c" and item.slot == "model"
    )
    assert inherited.distance == 2


def test_transparent_selector_exposes_its_selected_origin_provider() -> None:
    """Preserve authored selector priority without inheriting the selector transform."""

    provider = cube_document(
        "SelectorProvider",
        nodes={
            "first": {"class_type": "CheckpointLoader", "inputs": {}},
            "second": {"class_type": "CheckpointLoader", "inputs": {}},
            "selector": {
                "class_type": "AnySelector",
                "inputs": {"any_0": ["first", 0], "any_1": ["second", 0]},
            },
            "use": {
                "class_type": "ModelConsumer",
                "inputs": {"model": ["selector", 0]},
            },
        },
        definitions={
            **_resource_definitions(),
            "AnySelector": {
                "input": {"required": {"any_0": ["MODEL"], "any_1": ["MODEL"]}},
                "output": ["*"],
            },
        },
    )

    result = _resolve(
        {"cube-a": provider, "cube-b": consumer_document()},
        (_edge("cube-a", "cube-b"),),
    )

    assert _inputs(result.prompt["cube-b:model_use"])["model"] == ["cube-a:first", 0]


def test_disabled_local_provider_does_not_block_live_upstream_inheritance() -> None:
    """Resolve execution modes before evaluating local provider precedence."""

    target = consumer_document()
    implementation = target["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    nodes["disabled"] = {"class_type": "CheckpointLoader", "inputs": {}, "mode": 2}
    nodes["disabled_use"] = {
        "class_type": "ModelConsumer",
        "inputs": {"model": ["disabled", 0]},
    }

    result = _resolve(
        {"cube-a": provider_document(), "cube-b": target},
        (_edge("cube-a", "cube-b"),),
    )

    assert "cube-b:disabled" not in result.prompt
    assert _inputs(result.prompt["cube-b:model_use"])["model"] == [
        "cube-a:checkpoint",
        0,
    ]


def test_explicit_nonresource_type_prevents_name_based_inheritance() -> None:
    """Never inject MODEL into fields such as an upscale model-name selector."""

    target = cube_document(
        "TypedConsumer",
        nodes={
            "load_upscale_model": {
                "class_type": "UpscaleModelLoader",
                "inputs": {"model_name": None},
            }
        },
        definitions={
            **_resource_definitions(),
            "UpscaleModelLoader": {
                "input": {"required": {"model_name": ["LIST"]}},
                "output": ["UPSCALE_MODEL"],
            },
        },
    )

    result = _resolve(
        {"cube-a": provider_document(), "cube-b": target},
        (_edge("cube-a", "cube-b"),),
    )

    assert _inputs(result.prompt["cube-b:load_upscale_model"])["model_name"] is None


def test_equally_near_origins_fail_closed_instead_of_guessing() -> None:
    """Improve native branch behavior with a stable ambiguity diagnostic."""

    target = consumer_document("C")
    implementation = target["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    inputs = implementation["inputs"]
    assert isinstance(nodes, dict)
    assert isinstance(inputs, dict)
    nodes["__topology_other__"] = {
        "class_type": "TopologyRelay",
        "inputs": {"resource": "C-other"},
    }
    inputs["input.other"] = {
        "kind": "input",
        "targets": [["__topology_other__", "resource"]],
    }
    with pytest.raises(CubeInheritanceError) as captured:
        _resolve(
            {
                "cube-a": provider_document("A"),
                "cube-b": provider_document("B"),
                "cube-c": target,
            },
            (
                _edge("cube-a", "cube-c"),
                _edge("cube-b", "cube-c", target_binding="input.other"),
            ),
        )

    assert captured.value.code == "execution.inheritance.ambiguous_provider"
    assert captured.value.target_instance_id == "cube-c"
    assert captured.value.slot == "model"
    assert captured.value.provider_instance_ids == ("cube-a", "cube-b")


def test_disconnected_and_loose_bridged_cubes_do_not_inherit() -> None:
    """Use only accepted Cube-to-Cube edges as the special semantic boundary."""

    result = _resolve(
        {"cube-a": provider_document(), "cube-b": consumer_document()},
        (),
    )

    assert _inputs(result.prompt["cube-b:model_use"])["model"] is None
    assert result.inherited_bindings == ()


def test_outer_bypass_preserves_inheritance_but_disable_is_a_barrier() -> None:
    """Align special resources with the root Cube's native execution mode."""

    documents = {
        "cube-a": provider_document(),
        "cube-b": consumer_document("Middle"),
        "cube-c": consumer_document("Target"),
    }
    edges = (_edge("cube-a", "cube-b"), _edge("cube-b", "cube-c"))

    bypassed = _resolve(documents, edges, modes={"cube-b": 4})
    disabled = _resolve(documents, edges, modes={"cube-b": 2})

    assert _inputs(bypassed.prompt["cube-c:model_use"])["model"] == [
        "cube-a:checkpoint",
        0,
    ]
    inherited = next(
        item
        for item in bypassed.inherited_bindings
        if item.target_instance_id == "cube-c" and item.slot == "model"
    )
    assert inherited.distance == 2
    assert _inputs(disabled.prompt["cube-c:model_use"])["model"] is None


def _resolve(
    documents: dict[str, dict[str, object]],
    proximity: tuple[ProximityConnection, ...],
    *,
    modes: Mapping[str, int] | None = None,
) -> InheritanceResult:
    """Lower and resolve one fixture through production owners."""

    workflow = read_canonical_workflow(cube_workflow(documents, modes=modes))
    topology = build_cube_topology(workflow, proximity)
    lowered = NativeCubeWorkflowLowerer().lower(workflow, topology)
    return CubeInheritanceResolver().resolve(lowered, topology)


def _edge(
    source: str,
    target: str,
    *,
    target_binding: str = "input.resource",
) -> ProximityConnection:
    """Create one semantic Cube-only edge without presentation coordinates."""

    return ProximityConnection(
        source=CubeBoundaryEndpoint(source, "output.resource"),
        target=CubeBoundaryEndpoint(target, target_binding),
    )


def _inputs(node: Mapping[str, object]) -> Mapping[str, object]:
    """Narrow one inherited prompt input object for contract assertions."""

    inputs = node.get("inputs")
    assert isinstance(inputs, Mapping)
    return inputs


def _resource_definitions() -> dict[str, object]:
    """Copy the fixture definitions through an authored Cube document."""

    document = provider_document()
    implementation = document["implementation"]
    assert isinstance(implementation, Mapping)
    definitions = implementation["definitions"]
    assert isinstance(definitions, Mapping)
    return dict(definitions)
