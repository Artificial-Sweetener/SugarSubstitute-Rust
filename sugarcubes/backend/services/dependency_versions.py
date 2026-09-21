#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Evaluate dependency version requirements without inspecting the host."""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from typing import Any

from .dependency_semver import semver_at_least, semver_key
from .dependency_version_types import (
    CubeDependencyRequirement,
    DependencyStatus,
    GitContains,
    InstalledDependency,
    VersionKind,
    VersionRequirementPolicy,
)

_SEMVER_RE = re.compile(r"^\d+(?:\.\d+){1,3}(?:[-+][0-9A-Za-z.-]+)?$")
_GIT_SHA_RE = re.compile(r"^[0-9a-fA-F]{7,40}$")


def classify_version(value: str) -> VersionKind:
    """Classify a cube `ver` token without guessing arbitrary ordering."""

    normalized = _normalize_text(value)
    if not normalized:
        return "missing"
    if _GIT_SHA_RE.fullmatch(normalized):
        return "git_sha"
    if _SEMVER_RE.fullmatch(normalized):
        return "semver"
    return "unknown"


def build_dependency_version_plan(
    *,
    grouped_requirements: Mapping[str, Sequence[CubeDependencyRequirement]],
    installed_by_key: Mapping[str, InstalledDependency],
    git_contains: GitContains,
) -> list[dict[str, Any]]:
    """Project grouped requirements and installed evidence into a repair plan."""

    return [
        _version_plan_item(
            node_requirements=node_requirements,
            installed=installed_by_key.get(key),
            git_contains=git_contains,
        )
        for key, node_requirements in sorted(
            grouped_requirements.items(),
            key=lambda item: item[0],
        )
    ]


def _version_plan_item(
    *,
    node_requirements: Sequence[CubeDependencyRequirement],
    installed: InstalledDependency | None,
    git_contains: GitContains,
) -> dict[str, Any]:
    """Collapse one node's requirements and installed state into a plan item."""

    node_id = node_requirements[0].node_id
    kinds = {
        requirement.version_kind
        for requirement in node_requirements
        if requirement.version_kind != "missing"
    }
    required_version = _required_version(
        node_requirements,
        installed=installed,
        git_contains=git_contains,
    )
    required_kind = classify_version(required_version)
    exact_required = any(
        requirement.version_policy == "exact" for requirement in node_requirements
    )
    required_policy: VersionRequirementPolicy = "exact" if exact_required else "minimum"
    conflicts = _requirement_conflicts(
        node_requirements=node_requirements,
        installed=installed,
        git_contains=git_contains,
    )
    if conflicts:
        status: DependencyStatus = "version_conflict"
        repairable = False
        installed_version = installed.installed_version if installed is not None else ""
        installed_kind: VersionKind = (
            installed.version_kind if installed is not None else "missing"
        )
        remediation = "Resolve conflicting cube version requirements before repair."
    elif installed is None:
        status = "missing"
        repairable = True
        installed_version = ""
        installed_kind = "missing"
        remediation = "Install the required custom node."
    elif not kinds:
        status = "satisfied"
        repairable = False
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = ""
    elif required_kind == "semver":
        status = _semver_status(
            required_version,
            installed,
            policy=required_policy,
        )
        repairable = status not in {"satisfied", "blocked"}
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = _remediation_for_status(status)
    elif required_kind == "git_sha":
        status = _git_status(
            required_version,
            installed,
            git_contains=git_contains,
        )
        repairable = status in {"installed_commit_not_descendant"}
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = _remediation_for_status(status)
    else:
        status = "not_comparable"
        repairable = False
        installed_version = installed.installed_version
        installed_kind = installed.version_kind
        remediation = "Cube requirement version is not comparable."

    return {
        "nodeId": node_id,
        "displayName": node_id,
        "requiredVersion": required_version,
        "requiredVersionKind": required_kind,
        "requiredVersionPolicy": required_policy,
        "installedVersion": installed_version,
        "installedVersionKind": installed_kind,
        "installedEvidence": installed.to_payload() if installed is not None else None,
        "status": status,
        "repairable": repairable,
        "restartRequiredAfterRepair": status != "satisfied" and repairable,
        "requiredByPacks": _unique_sorted(
            requirement.pack_ref for requirement in node_requirements
        ),
        "requiredByCubeIds": _unique_sorted(
            requirement.cube_id for requirement in node_requirements
        ),
        "requiredByNodes": _unique_sorted(
            requirement.node_name for requirement in node_requirements
        ),
        "requirements": [requirement.to_payload() for requirement in node_requirements],
        "conflicts": conflicts,
        "remediation": remediation,
    }


def _requirement_conflicts(
    *,
    node_requirements: Sequence[CubeDependencyRequirement],
    installed: InstalledDependency | None,
    git_contains: GitContains,
) -> list[dict[str, Any]]:
    """Return conflicts that make a requirement group unsafe to compare."""

    kinds = {
        requirement.version_kind
        for requirement in node_requirements
        if requirement.version_kind not in {"missing", "unknown"}
    }
    exact_versions = _unique_sorted(
        requirement.required_version
        for requirement in node_requirements
        if requirement.version_kind == "semver"
        and requirement.version_policy == "exact"
    )
    if len(exact_versions) > 1:
        return [{"reason": "conflicting_exact_versions", "versions": exact_versions}]
    if exact_versions:
        minimum_versions = [
            requirement.required_version
            for requirement in node_requirements
            if requirement.version_kind == "semver"
            and requirement.version_policy == "minimum"
        ]
        if minimum_versions:
            strongest_minimum = max(minimum_versions, key=semver_key)
            if semver_key(strongest_minimum) > semver_key(exact_versions[0]):
                return [
                    {
                        "reason": "exact_version_below_minimum",
                        "versions": [exact_versions[0], strongest_minimum],
                    }
                ]
        return []
    if "semver" in kinds:
        return []
    if len(kinds) > 1:
        return [
            {
                "reason": "mixed_version_kinds",
                "versions": _unique_sorted(
                    requirement.required_version for requirement in node_requirements
                ),
            }
        ]
    if (
        kinds == {"git_sha"}
        and installed is not None
        and installed.source_kind == "git"
        and not installed.dirty
    ):
        git_versions = _unique_sorted(
            requirement.required_version
            for requirement in node_requirements
            if requirement.version_kind == "git_sha"
        )
        strongest = _strongest_git_requirement(
            node_requirements,
            source_path=installed.source_path,
            git_contains=git_contains,
        )
        if len(git_versions) > 1 and strongest is None:
            return [
                {
                    "reason": "divergent_git_requirements",
                    "versions": git_versions,
                }
            ]
    return []


def _required_version(
    requirements: Sequence[CubeDependencyRequirement],
    *,
    installed: InstalledDependency | None,
    git_contains: GitContains,
) -> str:
    """Return the strongest required version when it can be chosen locally."""

    semver_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "semver"
    ]
    if semver_versions:
        exact_versions = [
            requirement.required_version
            for requirement in requirements
            if requirement.version_kind == "semver"
            and requirement.version_policy == "exact"
        ]
        if exact_versions:
            return _unique_sorted(exact_versions)[0]
        return max(semver_versions, key=semver_key)
    git_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "git_sha"
    ]
    if git_versions:
        if (
            installed is not None
            and installed.source_kind == "git"
            and not installed.dirty
        ):
            strongest = _strongest_git_requirement(
                requirements,
                source_path=installed.source_path,
                git_contains=git_contains,
            )
            if strongest is not None:
                return strongest
        return _unique_sorted(git_versions)[-1]
    unknown_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "unknown"
    ]
    return unknown_versions[0] if unknown_versions else ""


def _semver_status(
    required_version: str,
    installed: InstalledDependency,
    *,
    policy: VersionRequirementPolicy,
) -> DependencyStatus:
    """Return semver readiness for one installed dependency."""

    if installed.source_kind == "git" and installed.dirty:
        return "blocked"
    if installed.version_kind == "missing":
        return "installed_version_unknown"
    if installed.version_kind != "semver":
        return "not_comparable"
    if policy == "exact":
        if installed.installed_version == required_version:
            return "satisfied"
        return "installed_version_mismatch"
    if semver_at_least(installed.installed_version, required_version):
        return "satisfied"
    return "installed_version_too_old"


def _git_status(
    required_version: str,
    installed: InstalledDependency,
    *,
    git_contains: GitContains,
) -> DependencyStatus:
    """Return git commit readiness for one installed dependency."""

    if installed.source_kind != "git":
        return "installed_version_unknown"
    if installed.dirty:
        return "blocked"
    installed_commit = installed.git_head or installed.installed_version
    if classify_version(installed_commit) != "git_sha":
        return "installed_version_unknown"
    if required_version == installed_commit:
        return "satisfied"
    if git_contains(
        installed.source_path,
        required_version,
        installed_commit,
    ):
        return "satisfied"
    return "installed_commit_not_descendant"


def _strongest_git_requirement(
    requirements: Sequence[CubeDependencyRequirement],
    *,
    source_path: str,
    git_contains: GitContains,
) -> str | None:
    """Return the unique required commit containing every other required SHA."""

    versions = _unique_sorted(
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "git_sha"
    )
    if not versions:
        return None
    strongest = [
        candidate
        for candidate in versions
        if all(
            other == candidate or git_contains(source_path, other, candidate)
            for other in versions
        )
    ]
    return strongest[0] if len(strongest) == 1 else None


def _remediation_for_status(status: DependencyStatus) -> str:
    """Return a short user-facing remediation for one version status."""

    if status == "installed_version_too_old":
        return "Update the installed custom node to the required version or newer."
    if status == "installed_version_mismatch":
        return "Install the exact custom-node version required by the cube library."
    if status == "installed_commit_not_descendant":
        return "Update the installed git checkout to a commit containing the required cube commit."
    if status == "installed_version_unknown":
        return "Installed custom-node version could not be proven safely."
    if status == "blocked":
        return "Installed custom-node checkout is dirty or otherwise unsafe to mutate."
    if status == "not_comparable":
        return "Installed and required versions are not safely comparable."
    return ""


def _unique_sorted(values: Any) -> list[str]:
    """Return non-empty unique strings sorted case-insensitively."""

    return sorted(
        {value.strip() for value in values if isinstance(value, str) and value.strip()},
        key=str.casefold,
    )


def _normalize_text(value: object) -> str:
    """Return a stripped string or an empty string."""

    return value.strip() if isinstance(value, str) else ""
