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
    blocked_version_items,
    select_install_items,
    select_version_items,
    skipped_install_items,
    skipped_version_items,
)
from sugarcubes.backend.services.dependency_requirements import (
    extract_versioned_requirements,
)
from sugarcubes.backend.services.dependency_requirement_fingerprint import (
    dependency_requirements_fingerprint,
)
from sugarcubes.backend.services.dependency_version_readiness import (
    dependency_version_readiness,
)
from sugarcubes.backend.services.dependency_version_types import (
    CubeDependencyRequirement,
    VersionRequirementPolicy,
)
from sugarcubes.backend.services.dependency_versions import classify_version
from tests.support.command_repository import CommandRepository


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
        "requiredVersionPolicy": "minimum",
        "requirementOrigin": "direct",
        "impliedByNodeId": "",
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
        repositories=CommandRepository(None),
    )

    plans = {item["nodeId"]: item for item in readiness["dependencyVersionPlan"]}
    assert plans["older-pack"]["status"] == "installed_version_too_old"
    assert plans["older-pack"]["repairable"] is True
    assert plans["current-pack"]["status"] == "satisfied"
    assert plans["current-pack"]["repairable"] is False
    assert plans["missing-pack"]["status"] == "missing"
    assert plans["missing-pack"]["restartRequiredAfterRepair"] is True


def test_readiness_reads_real_comfy_tracking_and_project_metadata(
    tmp_path: Path,
) -> None:
    """Read version identity from pyproject beside Comfy's tracked-file list."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "SimpleSyrup"
    installed.mkdir(parents=True)
    (installed / ".tracking").write_text(
        "__init__.py\nsimple_syrup/__init__.py",
        encoding="utf-8",
    )
    (installed / "pyproject.toml").write_text(
        "[project]\n"
        'name = "SimpleSyrup"\n'
        'version = "1.7.0"\n'
        "[project.urls]\n"
        'Repository = "https://github.com/Artificial-Sweetener/SimpleSyrup"\n',
        encoding="utf-8",
    )

    readiness = dependency_version_readiness(
        requirements=(_requirement("SimpleSyrup", "1.7.1"),),
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )

    plan = readiness["dependencyVersionPlan"][0]
    assert plan["status"] == "installed_version_too_old"
    assert plan["installedVersion"] == "1.7.0"
    assert plan["installedEvidence"]["sourceKind"] == "tracking"
    assert plan["installedEvidence"]["repositoryUrl"].endswith("/SimpleSyrup")


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
        repositories=CommandRepository(git_runner),
    )

    plans = {item["nodeId"]: item for item in readiness["dependencyVersionPlan"]}
    assert plans["clean-pack"]["status"] == "satisfied"
    assert plans["dirty-pack"]["status"] == "blocked"
    assert plans["dirty-pack"]["repairable"] is False
    assert (("merge-base", "--is-ancestor", "aaaaaaa", "bbbbbbb"), clean) in calls
    assert not any(call[0][0] == "merge-base" and call[1] == dirty for call in calls)


def test_sha_only_requirements_select_newest_required_commit_in_any_order(
    tmp_path: Path,
) -> None:
    """Select the unique descendant required SHA instead of input-order state."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "SimpleSyrup"
    git_dir = installed / ".git"
    git_dir.mkdir(parents=True)
    (git_dir / "HEAD").write_text("ddddddd", encoding="utf-8")
    rank = {"aaaaaaa": 1, "bbbbbbb": 2, "ccccccc": 3, "ddddddd": 4}

    def git_runner(args: list[str], *, cwd: Path) -> Any:
        assert cwd == installed
        if args == ["status", "--porcelain"]:
            return SimpleNamespace(returncode=0, stdout="")
        if args[:2] == ["merge-base", "--is-ancestor"]:
            ancestor, descendant = args[2:]
            return SimpleNamespace(
                returncode=0 if rank[ancestor] <= rank[descendant] else 1,
                stdout="",
            )
        return SimpleNamespace(returncode=0, stdout="")

    requirements = (
        _requirement("SimpleSyrup", "aaaaaaa"),
        _requirement("SimpleSyrup", "ccccccc"),
        _requirement("SimpleSyrup", "bbbbbbb"),
    )

    forward = dependency_version_readiness(
        requirements=requirements,
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(git_runner),
    )["dependencyVersionPlan"][0]
    reverse = dependency_version_readiness(
        requirements=tuple(reversed(requirements)),
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(git_runner),
    )["dependencyVersionPlan"][0]

    for plan in (forward, reverse):
        assert plan["requiredVersion"] == "ccccccc"
        assert plan["requiredVersionKind"] == "git_sha"
        assert plan["status"] == "satisfied"
        assert plan["conflicts"] == []


def test_semver_requirements_override_historical_requirement_kinds_in_any_order(
    tmp_path: Path,
) -> None:
    """Select the highest semver while retaining every requirement as evidence."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "SimpleSyrup"
    installed.mkdir(parents=True)
    (installed / ".tracking").write_text(
        json.dumps(
            {
                "version": "1.9.1",
                "repository": "https://github.com/Artificial-Sweetener/SimpleSyrup",
            }
        ),
        encoding="utf-8",
    )
    requirements = (
        _requirement("SimpleSyrup", "1.9.2"),
        _requirement("SimpleSyrup", "abcdef0"),
        _requirement("SimpleSyrup", "1.9.1"),
        _requirement("SimpleSyrup", ""),
        _requirement("SimpleSyrup", "floating"),
    )

    forward = dependency_version_readiness(
        requirements=requirements,
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )["dependencyVersionPlan"][0]
    reverse = dependency_version_readiness(
        requirements=tuple(reversed(requirements)),
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )["dependencyVersionPlan"][0]

    for plan in (forward, reverse):
        assert plan["requiredVersion"] == "1.9.2"
        assert plan["requiredVersionKind"] == "semver"
        assert plan["status"] == "installed_version_too_old"
        assert plan["repairable"] is True
        assert plan["conflicts"] == []
        assert {item["requiredVersion"] for item in plan["requirements"]} == {
            "1.9.1",
            "1.9.2",
            "abcdef0",
            "",
            "floating",
        }


def test_semver_repair_preserves_dirty_git_checkout(tmp_path: Path) -> None:
    """Refuse automatic semver repair when it would overwrite authored Git state."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "SimpleSyrup"
    git_dir = installed / ".git"
    git_dir.mkdir(parents=True)
    (git_dir / "HEAD").write_text("abcdef0", encoding="utf-8")

    def git_runner(args: list[str], *, cwd: Path) -> Any:
        assert cwd == installed
        if args == ["status", "--porcelain"]:
            return SimpleNamespace(returncode=0, stdout=" M authored.py")
        return SimpleNamespace(returncode=0, stdout="")

    plan = dependency_version_readiness(
        requirements=(_requirement("SimpleSyrup", "1.9.2"),),
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(git_runner),
    )["dependencyVersionPlan"][0]

    assert plan["requiredVersion"] == "1.9.2"
    assert plan["status"] == "blocked"
    assert plan["repairable"] is False
    assert plan["restartRequiredAfterRepair"] is False


def test_requirement_fingerprint_is_order_independent_and_version_sensitive() -> None:
    """Expose a stable change token for event-driven dependency reconciliation."""

    requirements = (
        _requirement("SimpleSyrup", "1.9.2"),
        _requirement("ComfyUI-Impact-Pack", "8.15.3"),
    )

    forward = dependency_requirements_fingerprint(requirements)
    reverse = dependency_requirements_fingerprint(tuple(reversed(requirements)))
    changed = dependency_requirements_fingerprint(
        (_requirement("SimpleSyrup", "1.9.3"), requirements[1])
    )

    assert forward == reverse
    assert changed != forward
    assert len(forward) == 64


def test_readiness_fingerprint_changes_only_with_required_dependency_state(
    tmp_path: Path,
) -> None:
    """Expose one stable token when a required installed version changes."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "SimpleSyrup"
    unrelated = custom_nodes_root / "unrelated-pack"
    installed.mkdir(parents=True)
    unrelated.mkdir()
    tracking_path = installed / ".tracking"
    tracking_path.write_text(
        json.dumps({"version": "1.9.1"}),
        encoding="utf-8",
    )
    requirements = (_requirement("SimpleSyrup", "1.9.2"),)

    outdated = dependency_version_readiness(
        requirements=requirements,
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )
    (unrelated / ".tracking").write_text(
        json.dumps({"version": "99.0.0"}),
        encoding="utf-8",
    )
    unrelated_changed = dependency_version_readiness(
        requirements=requirements,
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )
    tracking_path.write_text(
        json.dumps({"version": "1.9.2"}),
        encoding="utf-8",
    )
    satisfied = dependency_version_readiness(
        requirements=requirements,
        custom_nodes_root=custom_nodes_root,
        repositories=CommandRepository(None),
    )

    outdated_fingerprint = outdated["dependencyStateFingerprint"]
    assert len(outdated_fingerprint) == 64
    assert unrelated_changed["dependencyStateFingerprint"] == outdated_fingerprint
    assert satisfied["dependencyStateFingerprint"] != outdated_fingerprint


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


def test_blocked_projection_excludes_non_actionable_version_evidence() -> None:
    """Do not report uncomparable installed evidence as a failed repair."""

    version_plan = [
        {
            "nodeId": "dirty-pack",
            "status": "blocked",
            "repairable": False,
        },
        {
            "nodeId": "conflicting-pack",
            "status": "version_conflict",
            "repairable": False,
        },
        {
            "nodeId": "registry-pack",
            "status": "installed_version_unknown",
            "repairable": False,
        },
        {
            "nodeId": "opaque-pack",
            "status": "not_comparable",
            "repairable": False,
        },
    ]

    blocked = blocked_version_items(version_plan)

    assert [item["nodeId"] for item in blocked] == [
        "dirty-pack",
        "conflicting-pack",
    ]


def test_missing_dependency_with_conflicting_exact_versions_is_not_repairable(
    tmp_path: Path,
) -> None:
    """Refuse to choose an arbitrary release when exact requirements disagree."""

    plan = dependency_version_readiness(
        requirements=(
            _requirement("conflicting-pack", "3.0.0-beta.9", version_policy="exact"),
            _requirement("conflicting-pack", "3.0.0-beta.10", version_policy="exact"),
        ),
        custom_nodes_root=tmp_path / "custom_nodes",
        repositories=CommandRepository(None),
    )["dependencyVersionPlan"][0]

    assert plan["status"] == "version_conflict"
    assert plan["repairable"] is False
    assert plan["installedVersionKind"] == "missing"
    assert plan["conflicts"] == [
        {
            "reason": "conflicting_exact_versions",
            "versions": ["3.0.0-beta.10", "3.0.0-beta.9"],
        }
    ]


def test_exact_prerelease_requirement_rejects_newer_or_older_versions(
    tmp_path: Path,
) -> None:
    """Require the implied Prompt Control release exactly, including beta number."""

    custom_nodes_root = tmp_path / "custom_nodes"
    installed = custom_nodes_root / "comfyui-prompt-control"
    installed.mkdir(parents=True)
    tracking_path = installed / ".tracking"
    requirement = _requirement(
        "comfyui-prompt-control",
        "3.0.0-beta.3",
        version_policy="exact",
    )

    observed_statuses: list[str] = []
    for installed_version in (
        "3.0.0-beta.2",
        "3.0.0-beta.3",
        "3.0.0-beta.4",
        "3.0.0",
    ):
        tracking_path.write_text(
            json.dumps({"version": installed_version}),
            encoding="utf-8",
        )
        plan = dependency_version_readiness(
            requirements=(requirement,),
            custom_nodes_root=custom_nodes_root,
            repositories=CommandRepository(None),
        )["dependencyVersionPlan"][0]
        observed_statuses.append(plan["status"])
        assert plan["requiredVersionPolicy"] == "exact"

    assert observed_statuses == [
        "installed_version_mismatch",
        "satisfied",
        "installed_version_mismatch",
        "installed_version_mismatch",
    ]


def _requirement(
    node_id: str,
    version: str,
    *,
    version_policy: VersionRequirementPolicy = "minimum",
) -> CubeDependencyRequirement:
    """Build one characterized dependency requirement."""

    return CubeDependencyRequirement(
        node_id=node_id,
        required_version=version,
        version_kind=classify_version(version),
        cube_id="Example/Cubes/demo.cube",
        pack_ref="Example/Cubes",
        node_name=node_id,
        class_type="ExampleNode",
        source_path="demo.cube",
        default_base_repo=False,
        version_policy=version_policy,
    )
