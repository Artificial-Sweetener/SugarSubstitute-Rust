#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Characterize dependency version policy and installed evidence boundaries."""

from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from sugarcubes.backend.services.dependency_approval_policy import (
    select_install_items,
    select_version_items,
    skipped_install_items,
    skipped_version_items,
)
from sugarcubes.backend.services.dependency_requirements import (
    extract_versioned_requirements,
)
from sugarcubes.backend.services.dependency_version_readiness import (
    dependency_version_readiness,
)
from sugarcubes.backend.services.dependency_version_types import (
    CubeDependencyRequirement,
)


def test_extract_versioned_requirements_preserves_nodes_and_deduplicates_fallbacks() -> (
    None
):
    """Keep explicit node facts authoritative over matching module fallbacks."""

    payload = {
        "workflow": {
            "nodes": [
                {
                    "type": "ImpactNode",
                    "properties": {
                        "cnr_id": "ComfyUI_Impact-Pack",
                        "ver": "1.2.3",
                        "Node name for S&R": "Impact Detector",
                    },
                }
            ]
        },
        "definitions": {
            "ImpactNode": {
                "python_module": "custom_nodes.comfyui-impact-pack",
            },
            "OtherNode": {
                "python_module": "custom_nodes.comfyui-other-pack",
            },
            "SugarNode": {"python_module": "custom_nodes.sugarcubes"},
        },
    }

    requirements = extract_versioned_requirements(
        payload,
        cube_id="Example/Cubes/demo.cube",
        pack_ref="Example/Cubes",
        source_path="demo.cube",
        default_base_repo=False,
    )

    assert [requirement.node_id for requirement in requirements] == [
        "ComfyUI_Impact-Pack",
        "comfyui-other-pack",
    ]
    assert requirements[0].to_payload() == {
        "nodeId": "ComfyUI_Impact-Pack",
        "requiredVersion": "1.2.3",
        "requiredVersionKind": "semver",
        "cubeId": "Example/Cubes/demo.cube",
        "packRef": "Example/Cubes",
        "nodeName": "Impact Detector",
        "classType": "ImpactNode",
        "sourcePath": "demo.cube",
        "defaultBaseRepo": False,
    }
    assert requirements[1].version_kind == "missing"


def test_readiness_projects_semver_statuses_from_tracking_evidence(
    tmp_path: Path,
) -> None:
    """Preserve semantic-version comparison and repair projection."""

    custom_nodes_root = tmp_path / "custom_nodes"
    older = custom_nodes_root / "older-pack"
    current = custom_nodes_root / "current-pack"
    older.mkdir(parents=True)
    current.mkdir()
    (older / ".tracking").write_text(
        json.dumps({"version": "1.0.0", "repository": "https://example/older"}),
        encoding="utf-8",
    )
    (current / ".tracking").write_text(
        json.dumps({"version": "2.1.0", "repository": "https://example/current"}),
        encoding="utf-8",
    )

    readiness = dependency_version_readiness(
        requirements=(
            _requirement("older-pack", "1.2.0"),
            _requirement("current-pack", "2.0.0"),
            _requirement("missing-pack", "3.0.0"),
        ),
        custom_nodes_root=custom_nodes_root,
        git_runner=None,
    )

    plans = {item["nodeId"]: item for item in readiness["dependencyVersionPlan"]}
    assert plans["older-pack"]["status"] == "installed_version_too_old"
    assert plans["older-pack"]["repairable"] is True
    assert plans["current-pack"]["status"] == "satisfied"
    assert plans["current-pack"]["repairable"] is False
    assert plans["missing-pack"]["status"] == "missing"
    assert plans["missing-pack"]["restartRequiredAfterRepair"] is True


def test_readiness_uses_git_ancestry_and_blocks_dirty_checkouts(tmp_path: Path) -> None:
    """Preserve Git ancestry comparison and dirty-checkout safety."""

    custom_nodes_root = tmp_path / "custom_nodes"
    clean = custom_nodes_root / "clean-pack"
    dirty = custom_nodes_root / "dirty-pack"
    for checkout, head in ((clean, "bbbbbbb"), (dirty, "ddddddd")):
        git_dir = checkout / ".git"
        git_dir.mkdir(parents=True)
        (git_dir / "HEAD").write_text(head, encoding="utf-8")

    calls: list[tuple[tuple[str, ...], Path]] = []

    def git_runner(args: list[str], *, cwd: Path) -> Any:
        calls.append((tuple(args), cwd))
        if args == ["status", "--porcelain"]:
            return SimpleNamespace(
                returncode=0,
                stdout=" M changed.py" if cwd == dirty else "",
            )
        if args[:2] == ["merge-base", "--is-ancestor"]:
            return SimpleNamespace(returncode=0, stdout="")
        return SimpleNamespace(returncode=0, stdout="")

    readiness = dependency_version_readiness(
        requirements=(
            _requirement("clean-pack", "aaaaaaa"),
            _requirement("dirty-pack", "ccccccc"),
        ),
        custom_nodes_root=custom_nodes_root,
        git_runner=git_runner,
    )

    plans = {item["nodeId"]: item for item in readiness["dependencyVersionPlan"]}
    assert plans["clean-pack"]["status"] == "satisfied"
    assert plans["dirty-pack"]["status"] == "blocked"
    assert plans["dirty-pack"]["repairable"] is False
    assert (("merge-base", "--is-ancestor", "aaaaaaa", "bbbbbbb"), clean) in calls
    assert not any(call[0][0] == "merge-base" and call[1] == dirty for call in calls)


def test_approval_policy_keeps_baseline_silent_and_third_party_explicit() -> None:
    """Preserve baseline and third-party selection for install and version work."""

    install_plan = [
        {
            "nodeId": "baseline-pack",
            "installed": False,
            "installable": True,
            "confirmationRequired": False,
        },
        {
            "nodeId": "third-party-pack",
            "installed": False,
            "installable": True,
            "confirmationRequired": True,
        },
    ]
    version_plan = [
        {
            "nodeId": "baseline-pack",
            "status": "installed_version_too_old",
            "repairable": True,
            "requirements": [{"defaultBaseRepo": True}],
        },
        {
            "nodeId": "third-party-pack",
            "status": "installed_version_too_old",
            "repairable": True,
            "requirements": [{"defaultBaseRepo": False}],
        },
    ]

    selected_installs = select_install_items(
        install_plan,
        approval_policy="silent_baseline_only",
        approved_node_ids=(),
    )
    selected_versions = select_version_items(
        version_plan,
        approval_policy="silent_baseline_only",
        approved_node_ids=(),
    )

    assert [item["nodeId"] for item in selected_installs] == ["baseline-pack"]
    assert [
        item["nodeId"]
        for item in skipped_install_items(install_plan, selected_installs)
    ] == ["third-party-pack"]
    assert [item["nodeId"] for item in selected_versions] == ["baseline-pack"]
    assert [
        item["nodeId"]
        for item in skipped_version_items(version_plan, selected_versions)
    ] == ["third-party-pack"]


def _requirement(node_id: str, version: str) -> CubeDependencyRequirement:
    """Build one characterized dependency requirement."""

    return CubeDependencyRequirement(
        node_id=node_id,
        required_version=version,
        version_kind="git_sha" if len(version) == 7 and version.isalpha() else "semver",
        cube_id="Example/Cubes/demo.cube",
        pack_ref="Example/Cubes",
        node_name=node_id,
        class_type="ExampleNode",
        source_path="demo.cube",
        default_base_repo=False,
    )
