#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Aggregate cube-library dependency readiness."""

from __future__ import annotations

import hashlib
import json
import logging
from copy import deepcopy
from pathlib import Path
from time import perf_counter
from typing import Any

from ...instrumentation import log_diagnostic
from .cube_dependency_manifest import normalize_requirement_key
from .dependency_install_plan import (
    build_dependency_install_plan,
    installed_custom_nodes,
)
from .dependency_requirement_inventory import (
    DependencyRequirementInventory,
    DependencyRequirementLibrary,
)
from .dependency_version_readiness import dependency_version_readiness

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes cube library diagnostic"
_CACHE_TTL_SECONDS = 30.0


def _path_mtime_ns(path: Path) -> int:
    """Return a file timestamp suitable for cheap cache invalidation."""

    try:
        return path.stat().st_mtime_ns
    except OSError:
        return 0


class CubeLibraryReadinessService:
    """Project dependency readiness from one authoritative cube library."""

    def __init__(self, library: DependencyRequirementLibrary) -> None:
        """Initialize readiness projection for a library owner."""

        self._library = library
        self._requirements = DependencyRequirementInventory(library)
        self._cache: tuple[float, Path, str, dict[str, Any]] | None = None

    def library_readiness(self, custom_nodes_root: Path) -> dict[str, Any]:
        """Return target dependency readiness and install plan for enabled cubes."""

        started_at = perf_counter()
        phase_started_at = started_at
        phase_timings: dict[str, float] = {}
        custom_nodes_signature = self._cache_signature(custom_nodes_root)
        cached_payload = self._cached(
            custom_nodes_root=custom_nodes_root,
            custom_nodes_signature=custom_nodes_signature,
        )
        if cached_payload is not None:
            self._log(
                "sugarcubes_library_readiness_cache_hit",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            )
            return cached_payload

        def record_phase(name: str) -> None:
            """Record elapsed milliseconds for one readiness phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round((now - phase_started_at) * 1000, 3)
            phase_started_at = now

        requirements = self._requirements.collect()
        record_phase("dependency_requirement_sets")
        required = tuple(
            sorted(
                {record["node_id"] for record in requirements.records},
                key=str.casefold,
            )
        )
        installed = installed_custom_nodes(custom_nodes_root)
        record_phase("installed_custom_nodes")
        installed_keys = {normalize_requirement_key(slug) for slug in installed}
        missing = tuple(
            slug
            for slug in required
            if normalize_requirement_key(slug) not in installed_keys
        )
        install_plan = build_dependency_install_plan(
            requirement_records=requirements.records,
            installed=installed,
        )
        record_phase("dependency_install_plan")
        installable_missing = [
            item
            for item in install_plan
            if item["installed"] is False and item["installable"] is True
        ]
        version_readiness = dependency_version_readiness(
            requirements=requirements.version_requirements,
            custom_nodes_root=custom_nodes_root,
            git_runner=self._library.tracked_repo_service.git_runner,
        )
        record_phase("dependency_version_readiness")
        self._log(
            "sugarcubes_library_readiness_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            required_count=len(required),
            installed_count=len(installed),
            missing_count=len(missing),
            install_plan_count=len(install_plan),
            version_requirement_count=len(requirements.version_requirements),
            **phase_timings,
        )
        payload = {
            "schemaVersion": 1,
            "ready": not missing,
            "requiredCustomNodes": list(required),
            "missingCustomNodes": list(missing),
            "installedCustomNodes": [
                slug
                for slug in required
                if normalize_requirement_key(slug) in installed_keys
            ],
            "canInstall": bool(installable_missing),
            "installSupported": True,
            "catalogRevision": requirements.catalog_revision,
            "errors": [
                item["remediation"]
                for item in install_plan
                if item["installed"] is False and item["installable"] is False
            ],
            "installPlan": install_plan,
            "restartRequired": bool(missing),
            **version_readiness,
        }
        self._cache = (
            perf_counter(),
            custom_nodes_root.resolve(),
            custom_nodes_signature,
            deepcopy(payload),
        )
        return payload

    def _cached(
        self,
        *,
        custom_nodes_root: Path,
        custom_nodes_signature: str,
    ) -> dict[str, Any] | None:
        """Return recent readiness when source facts still match."""

        if self._cache is None:
            return None
        cached_at, cached_root, cached_signature, cached_payload = self._cache
        if perf_counter() - cached_at > _CACHE_TTL_SECONDS:
            self._cache = None
            return None
        if cached_root != custom_nodes_root.resolve():
            return None
        if cached_signature != custom_nodes_signature:
            self._cache = None
            return None
        return deepcopy(cached_payload)

    def _cache_signature(self, custom_nodes_root: Path) -> str:
        """Return cheap source facts that guard short-lived readiness reuse."""

        custom_node_facts: list[dict[str, Any]] = []
        try:
            entries = sorted(
                (entry for entry in custom_nodes_root.iterdir() if entry.is_dir()),
                key=lambda entry: entry.name.casefold(),
            )
        except OSError as exc:
            custom_node_facts.append(
                {"error": type(exc).__name__, "path": str(custom_nodes_root)}
            )
            entries = []
        for entry in entries:
            custom_node_facts.append(
                {
                    "name": entry.name,
                    "path_mtime_ns": _path_mtime_ns(entry),
                    "git_head_mtime_ns": _path_mtime_ns(entry / ".git" / "HEAD"),
                    "git_index_mtime_ns": _path_mtime_ns(entry / ".git" / "index"),
                    "tracking_mtime_ns": _path_mtime_ns(entry / ".tracking"),
                }
            )
        facts = {
            "customNodes": custom_node_facts,
            "dependencySources": self._requirements.source_signature(),
        }
        serialized = json.dumps(facts, sort_keys=True, separators=(",", ":"))
        return hashlib.sha256(serialized.encode("utf-8")).hexdigest()

    def _log(self, event: str, **fields: object) -> None:
        """Emit one structured library-readiness diagnostic."""

        log_diagnostic(_logger, _TRACE_MARKER, event, fields)
