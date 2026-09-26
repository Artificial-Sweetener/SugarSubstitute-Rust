#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Coordinate cube dependency synchronization, readiness, and repair."""

from __future__ import annotations

import logging
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from threading import Lock
from time import perf_counter
from typing import Any

from ...instrumentation.logger import log_diagnostic
from ..responses import BackendError
from .cube_library_service import CubeLibraryService
from .cube_metadata import normalize_metadata_string
from .dependency_acquisition import DependencyAcquirer
from .dependency_approval_policy import (
    DependencyApprovalPolicy,
    approval_policy_from_payload,
    approved_node_ids,
    blocked_version_items,
    select_install_items,
    select_version_items,
    skipped_install_items,
    skipped_version_items,
)
from .dependency_diagnostics import (
    diagnostics_from_repair_result,
    diagnostics_from_sync_errors,
)
from .dependency_installation import DependencyNodeInstaller
from .dependency_python_requirements import DependencyPythonRequirementsInstaller
from .dependency_registry_source import RegistrySourceResolver
from .dependency_repair_logging import (
    log_restart_required,
    log_source_selected,
    log_update_complete,
    log_update_failed,
    log_update_started,
    log_version_selected,
)
from .dependency_source_archive import TrustedSourceArchiveInstaller
from .dependency_source_git import RegistrySourceGitInstaller
from .dependency_version_repair import DependencyVersionRepairExecutor
from .tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes dependency acquisition diagnostic"


@dataclass(frozen=True)
class PackSyncResult:
    """Describe attempted Cube Pack sync work without making failures fatal."""

    synced_packs: list[dict[str, Any]]
    errors: list[dict[str, Any]]


class CubeDependencyService:
    """Coordinate cube dependency readiness and approved repair workflows."""

    def __init__(
        self,
        *,
        library_service: CubeLibraryService,
        tracked_repo_service: TrackedRepoService,
        custom_nodes_root: Path,
        acquirer: DependencyAcquirer | None = None,
        requirements_installer: DependencyPythonRequirementsInstaller | None = None,
        source_resolver: RegistrySourceResolver | None = None,
    ) -> None:
        """Initialize the service from the SugarCubes backend service graph."""

        self._library_service = library_service
        self._tracked_repo_service = tracked_repo_service
        self._custom_nodes_root = custom_nodes_root.resolve()
        dependency_requirements = (
            requirements_installer or DependencyPythonRequirementsInstaller()
        )
        dependency_acquirer = acquirer or DependencyAcquirer(
            source_resolver=source_resolver or RegistrySourceResolver(),
            git_installer=RegistrySourceGitInstaller(
                custom_nodes_root=self._custom_nodes_root,
                repositories=tracked_repo_service.repositories,
                requirements_installer=dependency_requirements,
            ),
            source_installer=TrustedSourceArchiveInstaller(
                custom_nodes_root=self._custom_nodes_root,
                requirements_installer=dependency_requirements,
            ),
        )
        self._node_installer = DependencyNodeInstaller(
            acquirer=dependency_acquirer,
        )
        self._version_repair = DependencyVersionRepairExecutor(
            acquirer=dependency_acquirer,
            repositories=tracked_repo_service.repositories,
            requirements_installer=dependency_requirements,
        )
        self._maintenance_lock = Lock()

    def readiness(self) -> dict[str, Any]:
        """Return dependency readiness without installing anything."""

        return self._library_service.library_readiness(self._custom_nodes_root)

    def sync_and_check(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Synchronize requested packs and recompute readiness atomically."""

        if not self._maintenance_lock.acquire(blocking=False):
            raise BackendError(
                "Cube dependency maintenance is already running",
                status=409,
                details={"reason": "maintenance_in_progress"},
            )
        try:
            self._tracked_repo_service.ensure_local_repo()
            sync_result = self._sync_requested_packs(payload.get("sync"))
            diagnostics = diagnostics_from_sync_errors(sync_result.errors)
            policy_value = payload.get("dependencyPolicy")
            dependency_policy = (
                policy_value if isinstance(policy_value, Mapping) else {}
            )
            readiness = self.readiness()
            repair_result: dict[str, Any] | None = None
            if bool(dependency_policy.get("repair")):
                repair_result = self.repair(
                    approval_policy=approval_policy_from_payload(dependency_policy),
                    approved_node_ids=approved_node_ids(dependency_policy),
                    sync_enabled_repos=False,
                )
                readiness = dict(repair_result.get("readinessAfter") or readiness)
                diagnostics.extend(diagnostics_from_repair_result(repair_result))
            return {
                "schemaVersion": 1,
                "diagnostics": diagnostics,
                "syncedPacks": sync_result.synced_packs,
                "syncErrors": sync_result.errors,
                "dependencyReadiness": readiness,
                "repairPlan": {
                    "schemaVersion": 1,
                    "installPlan": readiness.get("installPlan", []),
                    "dependencyVersionPlan": readiness.get("dependencyVersionPlan", []),
                },
                "repairResult": repair_result,
                "restartRequired": bool(
                    repair_result.get("restartRequired")
                    if repair_result is not None
                    else readiness.get("restartRequired")
                ),
                "errors": list(readiness.get("errors") or []),
            }
        finally:
            self._maintenance_lock.release()

    def repair(
        self,
        *,
        approval_policy: DependencyApprovalPolicy,
        approved_node_ids: Sequence[str] = (),
        sync_enabled_repos: bool = False,
    ) -> dict[str, Any]:
        """Install approved missing dependencies and return readiness changes."""

        started_at = perf_counter()
        phase_started_at = started_at
        phase_timings: dict[str, float] = {}

        def record_phase(name: str) -> None:
            """Record one repair phase and advance the local timing cursor."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round((now - phase_started_at) * 1000, 3)
            phase_started_at = now

        sync_errors = self._sync_enabled_repos(sync_enabled_repos)
        record_phase("repo_sync_ms")
        before = self.readiness()
        selected_items = select_install_items(
            before.get("installPlan"),
            approval_policy=approval_policy,
            approved_node_ids=approved_node_ids,
        )
        skipped = skipped_install_items(before.get("installPlan"), selected_items)
        record_phase("readiness_and_selection_ms")
        attempted, installed, failed = self._node_installer.install(selected_items)
        record_phase("missing_dependency_install_ms")

        after = self.readiness()
        version_items = select_version_items(
            after.get("dependencyVersionPlan"),
            approval_policy=approval_policy,
            approved_node_ids=approved_node_ids,
        )
        version_skipped = skipped_version_items(
            after.get("dependencyVersionPlan"), version_items
        )
        version_blocked = blocked_version_items(after.get("dependencyVersionPlan"))
        record_phase("post_install_readiness_ms")
        version_results: list[dict[str, Any]] = []
        version_failures: list[dict[str, Any]] = []
        for item in version_items:
            log_version_selected(item)
            log_source_selected(item, self._version_repair.selected_source(item))
            log_update_started(item)
            result = self._version_repair.repair(item)
            if result.get("returnCode") == 0:
                version_results.append(result)
                log_update_complete(item)
            else:
                version_failures.append(result)
                log_update_failed(item, result.get("reason"))
        for item in version_blocked:
            log_version_selected(item)
            log_update_failed(item, item.get("remediation") or item.get("status"))
        record_phase("version_repair_ms")
        if installed or version_results:
            log_restart_required()

        final_readiness = self.readiness()
        record_phase("final_readiness_ms")
        response_payload: dict[str, Any] = {
            "schemaVersion": 1,
            "syncErrors": sync_errors,
            "readinessBefore": before,
            "attemptedInstallPlan": attempted,
            "installedNodes": installed,
            "skippedNodes": skipped,
            "failedNodes": failed,
            "attemptedVersionPlan": version_items,
            "updatedNodes": version_results,
            "skippedVersionItems": version_skipped,
            "blockedVersionItems": version_blocked,
            "failedVersionItems": version_failures,
            "readinessAfter": final_readiness,
            "restartRequired": bool(installed or version_results),
        }
        response_payload["diagnostics"] = [
            *diagnostics_from_sync_errors(sync_errors),
            *diagnostics_from_repair_result(response_payload),
        ]
        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_dependency_repair_timing",
            {
                "selected_install_count": len(selected_items),
                "installed_count": len(installed),
                "failed_install_count": len(failed),
                "selected_version_count": len(version_items),
                "updated_count": len(version_results),
                "failed_version_count": len(version_failures),
                "total_duration_ms": round(
                    (perf_counter() - started_at) * 1000,
                    3,
                ),
                **phase_timings,
            },
        )
        return response_payload

    def _sync_enabled_repos(self, enabled: bool) -> list[dict[str, Any]]:
        """Attempt optional repository sync without preventing repair."""

        if not enabled:
            return []
        try:
            self._tracked_repo_service.sync_all_repos()
        except BackendError as exc:
            _logger.warning(
                "SugarCubes: continuing dependency repair after repo sync failure: %s",
                exc.message,
            )
            return [_repo_sync_error(exc)]
        except (OSError, RuntimeError, ValueError) as exc:
            _logger.warning(
                "SugarCubes: continuing dependency repair after repo sync failure",
                exc_info=True,
            )
            return [_unexpected_sync_error(exc)]
        return []

    def _sync_requested_packs(self, sync_payload: object) -> PackSyncResult:
        """Run a requested Cube Pack sync mode through library operations."""

        if not isinstance(sync_payload, Mapping):
            return PackSyncResult(synced_packs=[], errors=[])
        mode = normalize_metadata_string(sync_payload.get("mode"))
        if mode == "pack":
            owner = normalize_metadata_string(sync_payload.get("owner"))
            repo = normalize_metadata_string(sync_payload.get("repo"))
            if not owner or not repo:
                raise BackendError(
                    "Pack sync requires owner and repo",
                    status=400,
                    details={"reason": "missing_pack_ref"},
                )
            return self._attempt_sync_library_packs(({"owner": owner, "repo": repo},))
        if mode == "all":
            payload = self._library_service.sync_all_library_packs()
            packs_value = payload.get("packs")
            packs = packs_value if isinstance(packs_value, Sequence) else ()
            return PackSyncResult(
                synced_packs=[
                    dict(item) for item in packs if isinstance(item, Mapping)
                ],
                errors=[],
            )
        if mode == "default":
            repo_refs = (
                {
                    "owner": normalize_metadata_string(repo_entry.get("owner")),
                    "repo": normalize_metadata_string(repo_entry.get("repo")),
                }
                for repo_entry in self._tracked_repo_service.list_repos()["repos"]
                if repo_entry.get("enabled") and repo_entry.get("default_base_repo")
            )
            return self._attempt_sync_library_packs(repo_refs)
        if not mode:
            return PackSyncResult(synced_packs=[], errors=[])
        raise BackendError(
            "Unsupported dependency sync mode",
            status=400,
            details={"mode": mode},
        )

    def _attempt_sync_library_packs(
        self, repo_refs: Iterable[Mapping[str, str]]
    ) -> PackSyncResult:
        """Try pack syncs and preserve startup progress on failures."""

        synced: list[dict[str, Any]] = []
        errors: list[dict[str, Any]] = []
        for repo_ref in repo_refs:
            owner = normalize_metadata_string(repo_ref.get("owner"))
            repo = normalize_metadata_string(repo_ref.get("repo"))
            if not owner or not repo:
                continue
            try:
                synced.append(
                    self._library_service.sync_library_pack(owner=owner, repo=repo)
                )
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: continuing dependency maintenance after pack sync failure for %s/%s: %s",
                    owner,
                    repo,
                    exc.message,
                )
                errors.append(_pack_sync_error(owner=owner, repo=repo, error=exc))
        return PackSyncResult(synced_packs=synced, errors=errors)


def _repo_sync_error(error: BackendError) -> dict[str, Any]:
    """Return a JSON-safe nonfatal tracked-repo sync failure."""

    return {
        "status": error.status,
        "error": error.message,
        "details": error.details or {},
    }


def _unexpected_sync_error(error: Exception) -> dict[str, Any]:
    """Return a JSON-safe nonfatal unexpected sync failure."""

    return {
        "status": 500,
        "error": str(error).strip() or type(error).__name__,
        "details": {"exceptionType": type(error).__name__},
    }


def _pack_sync_error(
    *,
    owner: str,
    repo: str,
    error: BackendError,
) -> dict[str, Any]:
    """Return one structured non-fatal Cube Pack sync failure."""

    return {
        "owner": owner,
        "repo": repo,
        "repoRef": f"{owner}/{repo}",
        "status": error.status,
        "error": error.message,
        "details": error.details or {},
    }
