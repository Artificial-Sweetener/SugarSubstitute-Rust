#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Prove PNG workflow and SugarScript representations lower identically."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path

from PIL import Image

from sugarcubes.authoring import (
    NativeWorkflowImportPlan,
    project_native_workflow_plan,
)
from sugarcubes.backend.composition import BackendServices, build_backend_services
from sugarcubes.execution import (
    CubeExecutionRequest,
    CubeOptimizationOptions,
)
from .recipe_semantics import (
    canonical_hash,
    difference_count,
    leaf_count,
    prompt_output_semantics,
    semantic_prompt,
)


@dataclass(frozen=True)
class ModeProof:
    """Summarize one independently compiled artifact representation."""

    cube_count: int
    connection_count: int
    prompt_node_count: int
    semantic_field_count: int
    prompt_sha256: str
    semantic_sha256: str


@dataclass(frozen=True)
class ArtifactProof:
    """Record reproducible equality evidence for one recipe image."""

    file: str
    cubes: tuple[Mapping[str, object], ...]
    connections: tuple[Mapping[str, str], ...]
    workflow_only: ModeProof
    script_only: ModeProof
    combined: ModeProof
    optimized_prompt_sha256: str
    optimization_before_nodes: int
    optimization_after_nodes: int
    exact_representation_difference_count: int
    snapshot_file: str


def main() -> None:
    """Verify every requested image and write complete normalized snapshots."""

    arguments = _arguments()
    output_dir = arguments.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    services = build_backend_services(arguments.extension_root.resolve())
    proofs = [
        _verify_artifact(path.resolve(), output_dir, services)
        for path in arguments.paths
    ]
    summary = {
        "schema_version": 1,
        "valid": True,
        "artifacts": [asdict(proof) for proof in proofs],
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(
        json.dumps(summary, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    sys.stdout.write(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")


def _verify_artifact(
    path: Path,
    output_dir: Path,
    services: BackendServices,
) -> ArtifactProof:
    """Compile both authorities and compare their lowered execution semantics."""

    source, workflow = _read_metadata(path)
    workflow_plan = services.legacy_workflow_authoring.compile(workflow)
    source_result = services.sugarscript_authoring.compile(source)
    if source_result.plan is None:
        messages = "; ".join(item.message for item in source_result.diagnostics)
        raise ValueError(f"{path.name} SugarScript is invalid: {messages}")
    source_plan = source_result.plan
    _assert_plan_identity(path, workflow_plan, source_plan)

    workflow_prepared = services.execution.prepare(
        CubeExecutionRequest(
            workflow=project_native_workflow_plan(workflow_plan),
            optimization=CubeOptimizationOptions(enabled=False),
        )
    )
    source_prepared = services.execution.prepare(
        CubeExecutionRequest(
            workflow=project_native_workflow_plan(source_plan),
            optimization=CubeOptimizationOptions(enabled=False),
        )
    )
    workflow_prompt = _normalized_prompt(workflow_prepared.prompt, workflow_plan)
    source_prompt = _normalized_prompt(source_prepared.prompt, source_plan)
    workflow_semantics = semantic_prompt(workflow_prompt)
    source_semantics = semantic_prompt(source_prompt)
    _assert_equal(
        path, "unoptimized lowered semantics", workflow_semantics, source_semantics
    )

    workflow_optimized = services.execution.prepare(
        CubeExecutionRequest(workflow=project_native_workflow_plan(workflow_plan))
    )
    source_optimized = services.execution.prepare(
        CubeExecutionRequest(workflow=project_native_workflow_plan(source_plan))
    )
    workflow_optimized_prompt = _normalized_prompt(
        workflow_optimized.prompt, workflow_plan
    )
    source_optimized_prompt = _normalized_prompt(source_optimized.prompt, source_plan)
    workflow_output_semantics = prompt_output_semantics(
        semantic_prompt(workflow_optimized_prompt)
    )
    source_output_semantics = prompt_output_semantics(
        semantic_prompt(source_optimized_prompt)
    )
    _assert_equal(
        path,
        "optimized output semantics",
        workflow_output_semantics,
        source_output_semantics,
    )

    workflow_connections = _normalized_connections(workflow_plan)
    source_connections = _normalized_connections(source_plan)
    _assert_equal(path, "Cube connections", workflow_connections, source_connections)
    optimization = workflow_optimized.report.optimization
    if optimization is None:
        raise ValueError(f"{path.name} produced no optimization report")
    snapshot = {
        "schema_version": 1,
        "artifact": str(path),
        "cubes": _cube_inventory(workflow_plan),
        "connections": workflow_connections,
        "workflow_only_unoptimized_prompt": workflow_prompt,
        "script_only_unoptimized_prompt": source_prompt,
        "combined_unoptimized_prompt": source_prompt,
        "workflow_only_optimized_prompt": workflow_optimized_prompt,
        "script_only_optimized_prompt": source_optimized_prompt,
        "optimized_output_semantics": workflow_output_semantics,
        "optimization": asdict(optimization),
    }
    snapshot_path = output_dir / f"{path.stem}.semantic.json"
    snapshot_path.write_text(
        json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    workflow_proof = _mode_proof(workflow_plan, workflow_prompt, workflow_semantics)
    source_proof = _mode_proof(source_plan, source_prompt, source_semantics)
    combined_proof = ModeProof(**asdict(source_proof))
    return ArtifactProof(
        file=str(path),
        cubes=tuple(_cube_inventory(workflow_plan)),
        connections=tuple(workflow_connections),
        workflow_only=workflow_proof,
        script_only=source_proof,
        combined=combined_proof,
        optimized_prompt_sha256=canonical_hash(workflow_output_semantics),
        optimization_before_nodes=optimization.original_node_count,
        optimization_after_nodes=optimization.optimized_node_count,
        exact_representation_difference_count=difference_count(
            workflow_prompt, source_prompt
        ),
        snapshot_file=str(snapshot_path),
    )


def _read_metadata(path: Path) -> tuple[str, dict[str, object]]:
    """Read the two required PNG text records without interpreting image pixels."""

    with Image.open(path) as image:
        source = image.info.get("sugar_script")
        workflow_text = image.info.get("workflow")
    if not isinstance(source, str) or not source:
        raise ValueError(f"{path.name} has no SugarScript metadata")
    if not isinstance(workflow_text, str):
        raise ValueError(f"{path.name} has no workflow metadata")
    workflow = json.loads(workflow_text)
    if not isinstance(workflow, dict) or not all(
        isinstance(key, str) for key in workflow
    ):
        raise ValueError(f"{path.name} workflow metadata is not an object")
    return source, workflow


def _assert_plan_identity(
    path: Path,
    workflow: NativeWorkflowImportPlan,
    source: NativeWorkflowImportPlan,
) -> None:
    """Require the same named Cubes before comparing their lowered fields."""

    _assert_equal(
        path, "Cube inventory", _cube_inventory(workflow), _cube_inventory(source)
    )
    _assert_equal(
        path,
        "connection count",
        len(workflow.connections),
        len(source.connections),
    )


def _cube_inventory(plan: NativeWorkflowImportPlan) -> list[dict[str, object]]:
    """Return stable aliases, exact pins, and modes for every Cube instance."""

    result: list[dict[str, object]] = []
    for instance in plan.instances:
        document = _mapping(instance.payload.get("document"))
        result.append(
            {
                "alias": instance.alias,
                "cube_id": document.get("cube_id"),
                "version": document.get("version"),
                "bypassed": instance.bypassed,
            }
        )
    return result


def _normalized_connections(
    plan: NativeWorkflowImportPlan,
) -> list[dict[str, str]]:
    """Address every public edge through aliases rather than instance UUIDs."""

    aliases = {instance.instance_id: instance.alias for instance in plan.instances}
    return [
        {
            "source_alias": aliases[connection.source_instance_id],
            "source_binding": connection.source_binding,
            "target_alias": aliases[connection.target_instance_id],
            "target_binding": connection.target_binding,
        }
        for connection in plan.connections
    ]


def _normalized_prompt(
    prompt: Mapping[str, Mapping[str, object]],
    plan: NativeWorkflowImportPlan,
) -> dict[str, object]:
    """Replace representation-specific instance IDs throughout one prompt."""

    aliases = {instance.instance_id: instance.alias for instance in plan.instances}

    def normalize_id(value: str) -> str:
        for instance_id in sorted(aliases, key=len, reverse=True):
            if value == instance_id or value.startswith(f"{instance_id}:"):
                return f"{aliases[instance_id]}{value[len(instance_id):]}"
        return value

    def normalize(value: object) -> object:
        if isinstance(value, str):
            return normalize_id(value)
        if isinstance(value, Mapping):
            return {
                str(key): normalize(item)
                for key, item in sorted(value.items(), key=lambda pair: str(pair[0]))
            }
        if isinstance(value, Sequence) and not isinstance(
            value, (str, bytes, bytearray)
        ):
            return [normalize(item) for item in value]
        return value

    return {
        normalize_id(node_id): normalize(node)
        for node_id, node in sorted(prompt.items())
    }


def _mode_proof(
    plan: NativeWorkflowImportPlan,
    prompt: Mapping[str, object],
    semantics: Mapping[str, object],
) -> ModeProof:
    """Count all named input leaves represented by one lowered prompt."""

    return ModeProof(
        cube_count=len(plan.instances),
        connection_count=len(plan.connections),
        prompt_node_count=len(prompt),
        semantic_field_count=sum(leaf_count(value) for value in prompt.values()),
        prompt_sha256=canonical_hash(prompt),
        semantic_sha256=canonical_hash(semantics),
    )


def _assert_equal(path: Path, label: str, left: object, right: object) -> None:
    """Raise one artifact-scoped semantic mismatch with exact values."""

    if left != right:
        raise ValueError(
            f"{path.name} {label} differs: "
            f"workflow={json.dumps(left, ensure_ascii=False, default=str)} "
            f"source={json.dumps(right, ensure_ascii=False, default=str)}"
        )


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow one prepared payload object for proof generation."""

    if not isinstance(value, Mapping):
        raise ValueError("Prepared workflow instance has no document")
    return value


def _arguments() -> argparse.Namespace:
    """Parse explicit extension, output, and artifact paths."""

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--extension-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
    )
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("paths", nargs="+", type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    main()
