#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Prove Backend-compatible optimization under SugarCubes ownership rules."""

from __future__ import annotations

from copy import deepcopy

from sugarcubes.execution import (
    CubeBoundaryEndpoint,
    CubeOptimizationOptions,
    CubePromptOptimizer,
    CubeTopology,
    NodeOwner,
    ProximityConnection,
    build_cube_topology,
)
from sugarcubes.workflow import read_canonical_workflow
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)

RESOURCE_DEFINITIONS: dict[str, dict[str, object]] = {
    "TestModelClipLoader": {
        "input": {"required": {"ckpt_name": ["STRING"]}},
        "output": ["MODEL", "CLIP"],
    },
    "TestModelPatch": {
        "input": {"required": {"model": ["MODEL"], "strength": ["FLOAT"]}},
        "output": ["MODEL"],
    },
    "TestSampler": {
        "input": {"required": {"model": ["MODEL"]}},
        "output": ["LATENT"],
    },
}


def test_connected_cubes_share_equivalent_model_streams_like_backend() -> None:
    """Preserve the characterized recursive resource-stream rewrite."""

    prompt = _parallel_model_streams()
    original = deepcopy(prompt)
    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )

    assert prompt == original
    assert set(optimized) == {"1", "2", "5", "6"}
    assert optimized["6"]["inputs"] == {"model": ["2", 0]}
    assert [replacement.kind for replacement in report.replacements] == [
        "model_resource_stream"
    ]
    assert report.pass_counts == (
        ("bypass_empty_lazy_lora", 0),
        ("intern_pure_values", 0),
        ("intern_resource_streams", 1),
    )


def test_different_resource_root_identity_preserves_parallel_streams() -> None:
    """Keep the Backend exclusion for visibly different loaded resources."""

    prompt = _parallel_model_streams()
    prompt["3"]["inputs"] = {"ckpt_name": "different"}

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )

    assert optimized == prompt
    assert report.optimized is False


def test_disconnected_cubes_never_share_equivalent_streams() -> None:
    """Improve the legacy policy by making topology components hard barriers."""

    prompt = _parallel_model_streams()
    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=False),
        options=CubeOptimizationOptions(),
    )

    assert optimized == prompt
    assert report.optimized is False


def test_public_boundary_nodes_keep_their_execution_identity() -> None:
    """Never remove a node still addressed by a lowered Cube boundary binding."""

    prompt = _parallel_model_streams()
    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
        protected_node_ids=frozenset({"4"}),
    )

    assert optimized == prompt
    assert report.optimized is False


def test_loose_resource_dependencies_are_opaque_identity_barriers() -> None:
    """Never infer structural equivalence through loose workflow nodes."""

    prompt = _parallel_model_streams()
    owners = _two_cube_owners()
    owners["1"] = NodeOwner(None)
    owners["3"] = NodeOwner(None)

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )

    assert optimized == prompt
    assert report.optimized is False


def test_same_loose_dependency_identity_can_share_cube_owned_transforms() -> None:
    """Use a loose execution id opaquely without traversing or removing its node."""

    prompt = _parallel_model_streams()
    prompt["4"]["inputs"] = {"model": ["1", 0], "strength": 0.7}
    owners = _two_cube_owners()
    owners["1"] = NodeOwner(None)
    del prompt["3"]
    del owners["3"]

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )

    assert "1" in optimized
    assert "4" not in optimized
    assert optimized["6"]["inputs"] == {"model": ["2", 0]}
    assert [replacement.kind for replacement in report.replacements] == [
        "model_resource_stream"
    ]


def test_pure_values_share_only_inside_one_cube_component() -> None:
    """Keep characterized pure-value interning while excluding loose duplicates."""

    prompt: dict[str, dict[str, object]] = {
        "1": {"class_type": "PrimitiveStringMultiline", "inputs": {"value": "same"}},
        "2": {"class_type": "PrimitiveStringMultiline", "inputs": {"value": "same"}},
        "3": {"class_type": "PrimitiveStringMultiline", "inputs": {"value": "same"}},
        "4": {"class_type": "Sink", "inputs": {"first": ["1", 0], "second": ["2", 0]}},
        "5": {"class_type": "Sink", "inputs": {"value": ["3", 0]}},
    }
    owners = {
        "1": NodeOwner("cube-a"),
        "2": NodeOwner("cube-b"),
        "3": NodeOwner(None),
        "4": NodeOwner("cube-b"),
        "5": NodeOwner(None),
    }

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions={},
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(intern_resource_streams=False),
    )

    assert "2" not in optimized
    assert "3" in optimized
    assert optimized["4"]["inputs"] == {"first": ["1", 0], "second": ["1", 0]}
    assert [replacement.kind for replacement in report.replacements] == [
        "string_resource"
    ]


def test_empty_lazy_lora_is_bypassed_before_other_interning() -> None:
    """Preserve the Backend pass order and model/CLIP passthrough semantics."""

    prompt: dict[str, dict[str, object]] = {
        "1": {"class_type": "Loader", "inputs": {"name": "base"}},
        "2": {
            "class_type": "PCLazyLoraLoader",
            "inputs": {"model": ["1", 0], "clip": ["1", 1], "text": ""},
        },
        "3": {
            "class_type": "Consumer",
            "inputs": {"model": ["2", 0], "clip": ["2", 1]},
        },
    }
    owners = {node_id: NodeOwner("cube-a") for node_id in prompt}

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions={},
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )

    assert "2" not in optimized
    assert optimized["3"]["inputs"] == {"model": ["1", 0], "clip": ["1", 1]}
    assert [replacement.kind for replacement in report.replacements] == [
        "empty_lora_passthrough"
    ]
    assert report.pass_counts[0] == ("bypass_empty_lazy_lora", 1)


def test_lora_schedule_shares_without_collapsing_different_prompt_prose() -> None:
    """Preserve the characterized Prompt Control schedule/text distinction."""

    prompt = _lora_prompt("cat", "dog")
    owners = {node_id: NodeOwner("cube-a") for node_id in prompt}

    optimized, report = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions={},
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(intern_resource_streams=False),
    )

    assert _ids_by_class(optimized, "PCLazyLoraLoader") == ["5"]
    assert _ids_by_class(optimized, "PCLazyTextEncode") == ["7", "8"]
    assert optimized["8"]["inputs"] == {"clip": ["5", 1], "text": ["2", 0]}
    assert report.optimized is True


def test_identical_prompt_prose_shares_lora_and_conditioning_branches() -> None:
    """Preserve the strongest characterized Prompt Control equivalence."""

    prompt = _lora_prompt("cat", "cat")
    owners = {node_id: NodeOwner("cube-a") for node_id in prompt}

    optimized, _ = CubePromptOptimizer().optimize(
        prompt,
        node_owners=owners,
        node_definitions={},
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(intern_resource_streams=False),
    )

    assert _ids_by_class(optimized, "PCLazyLoraLoader") == ["5"]
    assert _ids_by_class(optimized, "PCLazyTextEncode") == ["7"]
    assert optimized["9"]["inputs"] == {"conditioning": ["7", 0]}


def test_optimizer_is_idempotent_and_honors_pass_options() -> None:
    """Run one fixed sequence and allow diagnosis to disable a pass only."""

    optimizer = CubePromptOptimizer()
    first, first_report = optimizer.optimize(
        _parallel_model_streams(),
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )
    second, second_report = optimizer.optimize(
        first,
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(),
    )
    disabled, disabled_report = optimizer.optimize(
        _parallel_model_streams(),
        node_owners=_two_cube_owners(),
        node_definitions=RESOURCE_DEFINITIONS,
        topology=_topology(connected=True),
        options=CubeOptimizationOptions(enabled=False),
    )

    assert first_report.optimized is True
    assert second == first
    assert second_report.optimized is False
    assert disabled == _parallel_model_streams()
    assert disabled_report.optimized is False


def _parallel_model_streams() -> dict[str, dict[str, object]]:
    """Return the characterized parallel model patch graph."""

    return {
        "1": {"class_type": "TestModelClipLoader", "inputs": {"ckpt_name": "base"}},
        "2": {
            "class_type": "TestModelPatch",
            "inputs": {"model": ["1", 0], "strength": 0.7},
        },
        "3": {"class_type": "TestModelClipLoader", "inputs": {"ckpt_name": "base"}},
        "4": {
            "class_type": "TestModelPatch",
            "inputs": {"model": ["3", 0], "strength": 0.7},
        },
        "5": {"class_type": "TestSampler", "inputs": {"model": ["2", 0]}},
        "6": {"class_type": "TestSampler", "inputs": {"model": ["4", 0]}},
    }


def _lora_prompt(
    first_subject: str, second_subject: str
) -> dict[str, dict[str, object]]:
    """Return the compact Substitute Backend Prompt Control fixture."""

    return {
        "0": {"class_type": "ModelAndClipProvider", "inputs": {"name": "base"}},
        "1": {
            "class_type": "PrimitiveStringMultiline",
            "inputs": {"value": f"{first_subject} <lora:same:1>"},
        },
        "2": {
            "class_type": "PrimitiveStringMultiline",
            "inputs": {"value": f"{second_subject} <lora:same:1>"},
        },
        "3": {"class_type": "RegexExtract", "inputs": _regex_inputs(["1", 0])},
        "4": {"class_type": "RegexExtract", "inputs": _regex_inputs(["2", 0])},
        "5": {
            "class_type": "PCLazyLoraLoader",
            "inputs": {"model": ["0", 0], "clip": ["0", 1], "text": ["3", 0]},
        },
        "6": {
            "class_type": "PCLazyLoraLoader",
            "inputs": {"model": ["0", 0], "clip": ["0", 1], "text": ["4", 0]},
        },
        "7": {
            "class_type": "PCLazyTextEncode",
            "inputs": {"clip": ["5", 1], "text": ["1", 0]},
        },
        "8": {
            "class_type": "PCLazyTextEncode",
            "inputs": {"clip": ["6", 1], "text": ["2", 0]},
        },
        "9": {"class_type": "Sink", "inputs": {"conditioning": ["8", 0]}},
    }


def _regex_inputs(source: list[object]) -> dict[str, object]:
    """Return the characterized RegexExtract schedule input shape."""

    return {
        "string": source,
        "regex_pattern": "<[^>]*>",
        "mode": "All Matches",
        "group_index": 1,
        "case_insensitive": False,
        "multiline": False,
        "dotall": False,
    }


def _ids_by_class(prompt: dict[str, dict[str, object]], class_type: str) -> list[str]:
    """Return deterministic node ids matching one prompt class."""

    return sorted(
        node_id
        for node_id, node in prompt.items()
        if node.get("class_type") == class_type
    )


def _two_cube_owners() -> dict[str, NodeOwner]:
    """Assign each parallel stream to one Cube."""

    return {
        "1": NodeOwner("cube-a"),
        "2": NodeOwner("cube-a"),
        "3": NodeOwner("cube-b"),
        "4": NodeOwner("cube-b"),
        "5": NodeOwner("cube-a"),
        "6": NodeOwner("cube-b"),
    }


def _topology(*, connected: bool) -> CubeTopology:
    """Return two deterministic Cube instances with optional direct topology."""

    workflow = read_canonical_workflow(
        cube_workflow(
            {
                "cube-a": image_passthrough_document("A"),
                "cube-b": image_passthrough_document("B"),
            }
        )
    )
    proximity = (
        ProximityConnection(
            source=CubeBoundaryEndpoint("cube-a", "output.image"),
            target=CubeBoundaryEndpoint("cube-b", "input.image"),
        ),
    )
    return build_cube_topology(workflow, proximity if connected else ())
