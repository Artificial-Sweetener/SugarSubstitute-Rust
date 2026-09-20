#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Coordinate dependency requirements, installed evidence, and version policy."""

from __future__ import annotations

import logging
from collections import defaultdict
from collections.abc import Sequence
from pathlib import Path
from time import perf_counter
from typing import Any

from ...instrumentation import log_diagnostic
from .dependency_git_inspection import DependencyGitAncestryInspector
from .dependency_inventory import installed_dependency_inventory
from .dependency_requirement_fingerprint import dependency_requirements_fingerprint
from .dependency_requirements import (
    comfy_runtime_readiness,
    is_external_custom_node_requirement,
    normalize_requirement_key,
)
from .dependency_version_types import CubeDependencyRequirement, GitRunner
from .dependency_versions import build_dependency_version_plan

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube library diagnostic"


def dependency_version_readiness(
    *,
    requirements: Sequence[CubeDependencyRequirement],
    custom_nodes_root: Path,
    git_runner: GitRunner | None,
) -> dict[str, Any]:
    """Build an additive version-readiness payload from cube requirements."""

    started_at = perf_counter()
    phase_started_at = started_at
    phase_timings: dict[str, float] = {}

    def record_phase(name: str) -> None:
        """Record elapsed milliseconds for one readiness phase."""

        nonlocal phase_started_at
        now = perf_counter()
        phase_timings[name] = round((now - phase_started_at) * 1000, 3)
        phase_started_at = now

    custom_node_requirements = [
        requirement
        for requirement in requirements
        if is_external_custom_node_requirement(requirement.node_id)
    ]
    record_phase("filter_custom_node_requirements")
    grouped: dict[str, list[CubeDependencyRequirement]] = defaultdict(list)
    for requirement in custom_node_requirements:
        grouped[normalize_requirement_key(requirement.node_id)].append(requirement)
    record_phase("group_requirements")
    installed = installed_dependency_inventory(
        custom_nodes_root,
        git_runner=git_runner,
        detailed_keys=frozenset(grouped),
    )
    record_phase("installed_dependency_inventory")
    installed_by_key = {
        normalize_requirement_key(item.folder_name): item for item in installed.values()
    }
    ancestry = DependencyGitAncestryInspector(git_runner)
    plan = build_dependency_version_plan(
        grouped_requirements=grouped,
        installed_by_key=installed_by_key,
        git_contains=ancestry.contains,
    )
    record_phase("build_version_plan")
    payload = {
        "versionedRequirementsSupported": True,
        "dependencyRequirementsFingerprint": dependency_requirements_fingerprint(
            custom_node_requirements
        ),
        "dependencyRequirements": [
            requirement.to_payload() for requirement in custom_node_requirements
        ],
        "installedDependencyEvidence": [
            item.to_payload()
            for item in sorted(
                installed.values(),
                key=lambda item: item.folder_name.casefold(),
            )
        ],
        "dependencyVersionPlan": plan,
        "comfyRuntimeReadiness": comfy_runtime_readiness(requirements),
    }
    record_phase("build_payload")
    log_diagnostic(
        _logger,
        _TRACE_MARKER,
        "sugarcubes_dependency_version_readiness_timing",
        {
            "total_duration_ms": round((perf_counter() - started_at) * 1000, 3),
            "requirement_count": len(requirements),
            "custom_node_requirement_count": len(custom_node_requirements),
            "installed_count": len(installed),
            "group_count": len(grouped),
            "plan_count": len(plan),
            "git_contains_check_count": ancestry.cached_check_count,
            **phase_timings,
        },
    )
    return payload
