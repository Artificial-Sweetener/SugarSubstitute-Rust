#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Install and repair custom-node dependencies required by cube libraries."""

from __future__ import annotations

import logging
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from threading import Lock
from typing import Any, Callable, Iterable, Literal, Mapping, Sequence

from ..responses import BackendError
from .cube_library_service import CubeLibraryService
from .cube_metadata import normalize_metadata_string
from .tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)
_CLI_TIMEOUT_SECONDS = 600

DependencyApprovalPolicy = Literal[
    "silent_baseline_only",
    "approved_node_ids",
    "approve_all",
]
DiagnosticSeverity = Literal["info", "warning", "error"]
SubprocessRunner = Callable[
    [Sequence[str], Path, int], subprocess.CompletedProcess[str]
]


@dataclass(frozen=True)
class ComfyCliResult:
    """Describe one Comfy CLI invocation used for dependency repair."""

    node_id: str
    command: tuple[str, ...]
    return_code: int
    stdout: str
    stderr: str

    def to_payload(self) -> dict[str, Any]:
        """Return a JSON-safe install result payload."""

        return {
            "nodeId": self.node_id,
            "command": list(self.command),
            "returnCode": self.return_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
        }


@dataclass(frozen=True)
class PackSyncResult:
    """Describe attempted Cube Pack sync work without making failures fatal."""

    synced_packs: list[dict[str, Any]]
    errors: list[dict[str, Any]]


class ComfyCliAdapter:
    """Run Comfy CLI through the selected Comfy workspace Python runtime."""

    def __init__(
        self,
        *,
        python_executable: Path | None = None,
        runner: SubprocessRunner | None = None,
    ) -> None:
        """Initialize the adapter with an explicit runtime and subprocess boundary."""

        self._python_executable = python_executable or Path(sys.executable)
        self._runner = runner or _run_subprocess

    def assert_available(self, workspace_path: Path) -> None:
        """Require `comfy_cli` to be importable in the selected runtime."""

        command = (
            str(self._python_executable),
            "-c",
            "import comfy_cli",
        )
        result = self._runner(command, workspace_path, 30)
        if result.returncode != 0:
            raise BackendError(
                "Comfy CLI is not available in the selected Comfy runtime",
                status=424,
                details={
                    "reason": "missing_comfy_cli",
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                },
            )

    def install_node(self, *, workspace_path: Path, node_id: str) -> ComfyCliResult:
        """Install one custom node through Comfy CLI."""

        normalized_node_id = normalize_metadata_string(node_id)
        if not normalized_node_id:
            raise BackendError("Custom node id is required", status=400)
        command = (
            str(self._python_executable),
            "-m",
            "comfy_cli",
            "--workspace",
            str(workspace_path),
            "--skip-prompt",
            "node",
            "install",
            "--exit-on-fail",
            normalized_node_id,
        )
        result = self._runner(command, workspace_path, _CLI_TIMEOUT_SECONDS)
        return ComfyCliResult(
            node_id=normalized_node_id,
            command=command,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
        )


class CubeDependencyService:
    """Coordinate cube dependency readiness and approved custom-node repair."""

    def __init__(
        self,
        *,
        library_service: CubeLibraryService,
        tracked_repo_service: TrackedRepoService,
        workspace_path: Path,
        custom_nodes_root: Path,
        cli_adapter: ComfyCliAdapter | None = None,
    ) -> None:
        """Initialize the service from the SugarCubes backend service graph."""

        self._library_service = library_service
        self._tracked_repo_service = tracked_repo_service
        self._workspace_path = workspace_path.resolve()
        self._custom_nodes_root = custom_nodes_root.resolve()
        self._cli_adapter = cli_adapter or ComfyCliAdapter()
        self._maintenance_lock = Lock()

    def readiness(self) -> dict[str, Any]:
        """Return dependency readiness without installing anything."""

        return self._library_service.library_readiness(self._custom_nodes_root)

    def sync_and_check(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Synchronize requested packs and recompute dependency readiness atomically."""

        if not self._maintenance_lock.acquire(blocking=False):
            raise BackendError(
                "Cube dependency maintenance is already running",
                status=409,
                details={"reason": "maintenance_in_progress"},
            )
        try:
            sync_result = self._sync_requested_packs(payload.get("sync"))
            diagnostics = _diagnostics_from_sync_errors(sync_result.errors)
            dependency_policy_value = payload.get("dependencyPolicy")
            dependency_policy = (
                dependency_policy_value
                if isinstance(dependency_policy_value, Mapping)
                else {}
            )
            readiness = self.readiness()
            repair_result: dict[str, Any] | None = None
            if bool(dependency_policy.get("repair")):
                repair_result = self.repair(
                    approval_policy=self._approval_policy_from_payload(
                        dependency_policy
                    ),
                    approved_node_ids=self._approved_node_ids(dependency_policy),
                    sync_enabled_repos=False,
                )
                readiness = dict(repair_result.get("readinessAfter") or readiness)
                diagnostics.extend(_diagnostics_from_repair_result(repair_result))
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
                    (repair_result or {}).get("restartRequired")
                    or readiness.get("restartRequired")
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
        """Install approved missing dependencies and return before/after readiness."""

        sync_errors: list[dict[str, Any]] = []
        if sync_enabled_repos:
            try:
                self._tracked_repo_service.sync_all_repos()
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: continuing dependency repair after repo sync "
                    "failure: %s",
                    exc.message,
                )
                sync_errors.append(_repo_sync_error(exc))
            except (OSError, RuntimeError, ValueError) as exc:
                _logger.warning(
                    "SugarCubes: continuing dependency repair after repo sync "
                    "failure",
                    exc_info=True,
                )
                sync_errors.append(_unexpected_sync_error(exc))
        before = self.readiness()
        selected_items = self._select_install_items(
            before.get("installPlan"),
            approval_policy=approval_policy,
            approved_node_ids=approved_node_ids,
        )
        installed: list[dict[str, Any]] = []
        failed: list[dict[str, Any]] = []
        skipped = self._skipped_items(before.get("installPlan"), selected_items)
        if selected_items:
            try:
                self._cli_adapter.assert_available(self._workspace_path)
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: Comfy CLI is unavailable for dependency repair: %s",
                    exc.message,
                )
                failed.extend(
                    _failed_install_result(item=item, error=exc)
                    for item in selected_items
                )
                selected_items = []

        for item in selected_items:
            node_id = normalize_metadata_string(item.get("nodeId"))
            try:
                install_result = self._cli_adapter.install_node(
                    workspace_path=self._workspace_path,
                    node_id=node_id,
                )
            except BackendError as exc:
                _logger.warning(
                    "SugarCubes: dependency install failed for %s: %s",
                    node_id,
                    exc.message,
                )
                failed.append(_failed_install_result(item=item, error=exc))
                continue
            except OSError as exc:
                _logger.exception(
                    "SugarCubes: failed to launch Comfy CLI for node %s", node_id
                )
                failed.append(
                    {
                        "nodeId": node_id,
                        "reason": str(exc),
                        "stdout": "",
                        "stderr": "",
                    }
                )
                continue
            install_payload = install_result.to_payload()
            if install_result.return_code == 0:
                installed.append(install_payload)
            else:
                failed.append(
                    {
                        **install_payload,
                        "reason": "Comfy CLI failed to install the custom node",
                    }
                )

        after = self.readiness()
        version_items = self._select_version_items(
            after.get("dependencyVersionPlan"),
            approval_policy=approval_policy,
            approved_node_ids=approved_node_ids,
        )
        version_results: list[dict[str, Any]] = []
        version_failures: list[dict[str, Any]] = []
        version_skipped = self._skipped_version_items(
            after.get("dependencyVersionPlan"), version_items
        )
        for item in version_items:
            result = self._repair_version_item(item)
            if result.get("returnCode") == 0:
                version_results.append(result)
            else:
                version_failures.append(result)
        final_readiness = self.readiness()
        response_payload: dict[str, Any] = {
            "schemaVersion": 1,
            "syncErrors": sync_errors,
            "readinessBefore": before,
            "attemptedInstallPlan": selected_items,
            "installedNodes": installed,
            "skippedNodes": skipped,
            "failedNodes": failed,
            "attemptedVersionPlan": version_items,
            "updatedNodes": version_results,
            "skippedVersionItems": version_skipped,
            "failedVersionItems": version_failures,
            "readinessAfter": final_readiness,
            "restartRequired": bool(installed or version_results),
        }
        response_payload["diagnostics"] = [
            *_diagnostics_from_sync_errors(sync_errors),
            *_diagnostics_from_repair_result(response_payload),
        ]
        return response_payload

    def _sync_requested_packs(self, sync_payload: object) -> PackSyncResult:
        """Run the requested Cube Pack sync mode through library-owned operations."""

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
            return self._attempt_sync_library_packs(
                ({"owner": owner, "repo": repo},),
            )
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
        """Try requested pack syncs and preserve startup progress on sync failures."""

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
                    "SugarCubes: continuing dependency maintenance after pack sync "
                    "failure for %s/%s: %s",
                    owner,
                    repo,
                    exc.message,
                )
                errors.append(_pack_sync_error(owner=owner, repo=repo, error=exc))
        return PackSyncResult(synced_packs=synced, errors=errors)

    def _approval_policy_from_payload(
        self,
        dependency_policy: Mapping[str, Any],
    ) -> DependencyApprovalPolicy:
        """Return repair approval policy from a sync-and-check request."""

        if bool(dependency_policy.get("approveAll")):
            return "approve_all"
        if bool(dependency_policy.get("baselineOnly")):
            return "silent_baseline_only"
        return "approved_node_ids"

    def _approved_node_ids(
        self,
        dependency_policy: Mapping[str, Any],
    ) -> tuple[str, ...]:
        """Return normalized approved node ids from a sync-and-check request."""

        approved_node_ids = dependency_policy.get("approvedNodeIds")
        if not isinstance(approved_node_ids, list):
            return ()
        return tuple(
            normalize_metadata_string(node_id)
            for node_id in approved_node_ids
            if normalize_metadata_string(node_id)
        )

    def _select_install_items(
        self,
        install_plan: object,
        *,
        approval_policy: DependencyApprovalPolicy,
        approved_node_ids: Sequence[str],
    ) -> list[dict[str, Any]]:
        """Return the plan items allowed by the requested approval policy."""

        approved = {normalize_metadata_string(node_id) for node_id in approved_node_ids}
        selected: list[dict[str, Any]] = []
        for item in _iter_plan_items(install_plan):
            if item.get("installed") is True or item.get("installable") is not True:
                continue
            node_id = normalize_metadata_string(item.get("nodeId"))
            confirmation_required = bool(item.get("confirmationRequired"))
            if approval_policy == "approve_all":
                selected.append(item)
            elif (
                approval_policy == "silent_baseline_only" and not confirmation_required
            ):
                selected.append(item)
            elif (
                approval_policy == "approved_node_ids"
                and confirmation_required
                and node_id in approved
            ):
                selected.append(item)
            elif (
                approval_policy == "approved_node_ids"
                and not confirmation_required
                and (not approved or node_id in approved)
            ):
                selected.append(item)
        return selected

    def _skipped_items(
        self, install_plan: object, selected_items: Sequence[Mapping[str, Any]]
    ) -> list[dict[str, Any]]:
        """Return missing items that were not selected for installation."""

        selected = {
            normalize_metadata_string(item.get("nodeId")) for item in selected_items
        }
        skipped: list[dict[str, Any]] = []
        for item in _iter_plan_items(install_plan):
            node_id = normalize_metadata_string(item.get("nodeId"))
            if item.get("installed") is True or node_id in selected:
                continue
            skipped.append(item)
        return skipped

    def _select_version_items(
        self,
        version_plan: object,
        *,
        approval_policy: DependencyApprovalPolicy,
        approved_node_ids: Sequence[str],
    ) -> list[dict[str, Any]]:
        """Return approved version plan items that can be repaired."""

        approved = {normalize_metadata_string(node_id) for node_id in approved_node_ids}
        selected: list[dict[str, Any]] = []
        for item in _iter_plan_items(version_plan):
            if item.get("status") in {"satisfied", "missing"}:
                continue
            if item.get("repairable") is not True:
                continue
            node_id = normalize_metadata_string(item.get("nodeId"))
            confirmation_required = _version_item_confirmation_required(item)
            if approval_policy == "approve_all":
                selected.append(item)
            elif (
                approval_policy == "silent_baseline_only" and not confirmation_required
            ):
                selected.append(item)
            elif (
                approval_policy == "approved_node_ids"
                and confirmation_required
                and node_id in approved
            ):
                selected.append(item)
            elif (
                approval_policy == "approved_node_ids"
                and not confirmation_required
                and (not approved or node_id in approved)
            ):
                selected.append(item)
        return selected

    def _skipped_version_items(
        self,
        version_plan: object,
        selected_items: Sequence[Mapping[str, Any]],
    ) -> list[dict[str, Any]]:
        """Return repairable version items that were not approved."""

        selected = {
            normalize_metadata_string(item.get("nodeId")) for item in selected_items
        }
        skipped: list[dict[str, Any]] = []
        for item in _iter_plan_items(version_plan):
            node_id = normalize_metadata_string(item.get("nodeId"))
            if item.get("status") in {"satisfied", "missing"} or node_id in selected:
                continue
            if item.get("repairable") is True:
                skipped.append(item)
        return skipped

    def _repair_version_item(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Repair one approved dependency version plan item."""

        status = normalize_metadata_string(item.get("status"))
        if status == "installed_commit_not_descendant":
            return self._checkout_required_git_commit(item)
        return self._reinstall_versioned_node(item)

    def _reinstall_versioned_node(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Use Comfy CLI to update or reinstall a versioned custom node."""

        node_id = normalize_metadata_string(item.get("nodeId"))
        evidence = item.get("installedEvidence")
        if not isinstance(evidence, Mapping) or not normalize_metadata_string(
            evidence.get("repositoryUrl")
        ):
            return _failed_version_result(
                node_id=node_id,
                operation="comfy_cli_install",
                reason="repository_provenance_missing",
            )
        try:
            self._cli_adapter.assert_available(self._workspace_path)
            result = self._cli_adapter.install_node(
                workspace_path=self._workspace_path,
                node_id=node_id,
            )
        except BackendError as exc:
            _logger.warning(
                "SugarCubes: Comfy CLI version repair failed for %s: %s",
                node_id,
                exc.message,
            )
            return {
                "nodeId": node_id,
                "operation": "comfy_cli_install",
                "returnCode": 1,
                "reason": exc.details.get("reason") or exc.message,
                "stdout": normalize_metadata_string(exc.details.get("stdout")),
                "stderr": normalize_metadata_string(exc.details.get("stderr")),
            }
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to launch Comfy CLI for version repair %s",
                node_id,
            )
            return {
                "nodeId": node_id,
                "operation": "comfy_cli_install",
                "returnCode": 1,
                "reason": str(exc),
                "stdout": "",
                "stderr": "",
            }
        payload = result.to_payload()
        return {
            **payload,
            "operation": "comfy_cli_install",
            "reason": (
                ""
                if result.return_code == 0
                else "Comfy CLI failed to update the custom node"
            ),
        }

    def _checkout_required_git_commit(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Fetch and checkout the approved required git commit when safe."""

        node_id = normalize_metadata_string(item.get("nodeId"))
        required_version = normalize_metadata_string(item.get("requiredVersion"))
        evidence = item.get("installedEvidence")
        if not isinstance(evidence, Mapping):
            return _failed_version_result(
                node_id=node_id,
                operation="git_checkout",
                reason="installed_evidence_missing",
            )
        source_path = Path(normalize_metadata_string(evidence.get("sourcePath")))
        repository_url = normalize_metadata_string(evidence.get("repositoryUrl"))
        if normalize_metadata_string(evidence.get("sourceKind")) != "git":
            return _failed_version_result(
                node_id=node_id,
                operation="git_checkout",
                reason="installed_source_not_git",
            )
        if bool(evidence.get("dirty")):
            return _failed_version_result(
                node_id=node_id,
                operation="git_checkout",
                reason="dirty_git_checkout",
            )
        if not repository_url:
            return _failed_version_result(
                node_id=node_id,
                operation="git_checkout",
                reason="repository_provenance_missing",
            )
        if not required_version:
            return _failed_version_result(
                node_id=node_id,
                operation="git_checkout",
                reason="required_version_missing",
            )
        git_runner = self._tracked_repo_service.git_runner
        commands = (
            ["fetch", "--all", "--tags"],
            ["cat-file", "-e", f"{required_version}^{{commit}}"],
            ["checkout", required_version],
        )
        for command in commands:
            try:
                result = git_runner(command, cwd=source_path)
            except (OSError, RuntimeError, ValueError) as exc:
                _logger.warning(
                    "SugarCubes: git version repair failed for %s",
                    node_id,
                    exc_info=True,
                )
                return {
                    "nodeId": node_id,
                    "operation": "git_checkout",
                    "command": command,
                    "returnCode": 1,
                    "reason": str(exc),
                    "stdout": "",
                    "stderr": "",
                }
            return_code = int(getattr(result, "returncode", 0) or 0)
            if return_code != 0:
                return {
                    "nodeId": node_id,
                    "operation": "git_checkout",
                    "command": command,
                    "returnCode": return_code,
                    "reason": "git_command_failed",
                    "stdout": normalize_metadata_string(getattr(result, "stdout", "")),
                    "stderr": normalize_metadata_string(getattr(result, "stderr", "")),
                }
        return {
            "nodeId": node_id,
            "operation": "git_checkout",
            "command": list(commands[-1]),
            "returnCode": 0,
            "reason": "",
            "stdout": "",
            "stderr": "",
        }


def _iter_plan_items(install_plan: object) -> list[dict[str, Any]]:
    """Coerce an install-plan payload into item dictionaries."""

    if not isinstance(install_plan, list):
        return []
    return [dict(item) for item in install_plan if isinstance(item, Mapping)]


def _diagnostic(
    *,
    code: str,
    severity: DiagnosticSeverity,
    title: str,
    message: str,
    details: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return one JSON-safe SugarCubes maintenance diagnostic."""

    return {
        "source": "SugarCubes",
        "code": code,
        "severity": severity,
        "title": title,
        "message": message,
        "details": _json_safe_details(details or {}),
    }


def _diagnostics_from_sync_errors(
    sync_errors: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Return startup diagnostics for non-fatal Cube Pack sync errors."""

    diagnostics: list[dict[str, Any]] = []
    for error in sync_errors:
        repo_ref = normalize_metadata_string(error.get("repoRef"))
        diagnostics.append(
            _diagnostic(
                code="base_cubes_sync_failed",
                severity="warning",
                title="Base-Cubes sync failed",
                message=(
                    "SugarCubes could not update Base-Cubes and is using the "
                    "local checkout."
                    if "Base-Cubes" in repo_ref
                    else "SugarCubes could not update one cube pack and is using local data."
                ),
                details=error,
            )
        )
    return diagnostics


def _diagnostics_from_repair_result(
    repair_result: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Return startup diagnostics from dependency repair outcomes."""

    diagnostics: list[dict[str, Any]] = []
    for item in _iter_plan_items(repair_result.get("failedNodes")):
        node_id = normalize_metadata_string(item.get("nodeId"))
        diagnostics.append(
            _diagnostic(
                code="sugarcubes_dependency_install_failed",
                severity="error",
                title="SugarCubes dependency install failed",
                message=(
                    f"{node_id} could not be installed automatically. Cube "
                    "workflows that require it may fail until it is repaired."
                    if node_id
                    else "A cube dependency could not be installed automatically."
                ),
                details=item,
            )
        )
    for item in _iter_plan_items(repair_result.get("failedVersionItems")):
        node_id = normalize_metadata_string(item.get("nodeId"))
        diagnostics.append(
            _diagnostic(
                code="sugarcubes_dependency_version_repair_failed",
                severity="error",
                title="SugarCubes dependency version repair failed",
                message=(
                    f"{node_id} could not be moved to the cube-required version."
                    if node_id
                    else "A cube dependency could not be moved to the required version."
                ),
                details=item,
            )
        )
    for item in _iter_plan_items(repair_result.get("skippedNodes")):
        if bool(item.get("confirmationRequired")):
            diagnostics.append(_approval_required_diagnostic(item))
    for item in _iter_plan_items(repair_result.get("skippedVersionItems")):
        if _version_item_confirmation_required(item):
            diagnostics.append(_approval_required_diagnostic(item))
    return diagnostics


def _approval_required_diagnostic(item: Mapping[str, Any]) -> dict[str, Any]:
    """Return a diagnostic for a dependency that cannot be repaired silently."""

    node_id = normalize_metadata_string(item.get("nodeId"))
    return _diagnostic(
        code="sugarcubes_dependency_needs_approval",
        severity="warning",
        title="SugarCubes dependency needs approval",
        message=(
            f"{node_id} needs user approval before SugarCubes can install or repair it."
            if node_id
            else "Some cube dependencies need user approval before SugarCubes can repair them."
        ),
        details=item,
    )


def _failed_install_result(
    *,
    item: Mapping[str, Any],
    error: BackendError,
) -> dict[str, Any]:
    """Return one failed install result from a structured backend error."""

    return {
        "nodeId": normalize_metadata_string(item.get("nodeId")),
        "reason": error.details.get("reason") or error.message,
        "stdout": normalize_metadata_string(error.details.get("stdout")),
        "stderr": normalize_metadata_string(error.details.get("stderr")),
        "status": error.status,
    }


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


def _json_safe_details(details: Mapping[str, Any]) -> dict[str, Any]:
    """Return a compact JSON-safe diagnostic details mapping."""

    safe: dict[str, Any] = {}
    for key, value in details.items():
        if isinstance(value, Mapping):
            safe[str(key)] = _json_safe_details(value)
        elif isinstance(value, list):
            safe[str(key)] = [
                (
                    _json_safe_details(item)
                    if isinstance(item, Mapping)
                    else _json_safe_scalar(item)
                )
                for item in value[:20]
            ]
        else:
            safe[str(key)] = _json_safe_scalar(value)
    return safe


def _json_safe_scalar(value: Any) -> object:
    """Return a JSON-safe scalar for diagnostic payloads."""

    if isinstance(value, str):
        return value[:4000]
    if isinstance(value, (bool, int, float)) or value is None:
        return value
    return str(value)[:4000]


def _version_item_confirmation_required(item: Mapping[str, Any]) -> bool:
    """Return whether one version repair item needs explicit user approval."""

    requirements = item.get("requirements")
    if not isinstance(requirements, list):
        return True
    baseline_values = [
        bool(requirement.get("defaultBaseRepo"))
        for requirement in requirements
        if isinstance(requirement, Mapping)
    ]
    return not baseline_values or not all(baseline_values)


def _failed_version_result(
    *,
    node_id: str,
    operation: str,
    reason: str,
) -> dict[str, Any]:
    """Return a failed version-repair result payload."""

    return {
        "nodeId": node_id,
        "operation": operation,
        "returnCode": 1,
        "reason": reason,
        "stdout": "",
        "stderr": "",
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


def _run_subprocess(
    command: Sequence[str], cwd: Path, timeout_seconds: int
) -> subprocess.CompletedProcess[str]:
    """Run one subprocess through an argument list with captured output."""

    return subprocess.run(
        list(command),
        cwd=str(cwd),
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
