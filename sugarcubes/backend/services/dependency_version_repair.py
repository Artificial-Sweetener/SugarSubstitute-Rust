#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Execute approved dependency version remediation."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .cube_metadata import normalize_metadata_string
from .dependency_acquisition import DependencyAcquirer
from .dependency_python_requirements import DependencyPythonRequirementsInstaller
from .dependency_version_types import GitRunner
from .dependency_versions import classify_version


class DependencyVersionRepairExecutor:
    """Apply version repair through acquisition policy or a safe Git checkout."""

    def __init__(
        self,
        *,
        acquirer: DependencyAcquirer,
        git_runner: GitRunner,
        requirements_installer: DependencyPythonRequirementsInstaller,
    ) -> None:
        """Initialize version repair with explicit execution adapters."""

        self._acquirer = acquirer
        self._git_runner = git_runner
        self._requirements_installer = requirements_installer

    def repair(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Repair one approved dependency version plan item."""

        status = normalize_metadata_string(item.get("status"))
        if status == "installed_commit_not_descendant":
            return self._checkout_required_git_ref(item)
        if _is_clean_semver_git_repair(item):
            required_version = normalize_metadata_string(item.get("requiredVersion"))
            return self._checkout_required_git_ref(
                item,
                checkout_ref=f"v{required_version}",
            )
        return self._reinstall_versioned_node(item)

    def selected_source(self, item: Mapping[str, Any]) -> str:
        """Return the acquisition path selected for one repair plan item."""

        status = normalize_metadata_string(item.get("status"))
        if status == "installed_commit_not_descendant" or _is_clean_semver_git_repair(
            item
        ):
            return "the installed clean Git checkout"
        return "Comfy Registry with authoritative repository fallback"

    def _reinstall_versioned_node(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Reacquire a versioned node through the shared acquisition policy."""

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
        return self._acquirer.acquire(item)

    def _checkout_required_git_ref(
        self,
        item: Mapping[str, Any],
        *,
        checkout_ref: str | None = None,
    ) -> dict[str, Any]:
        """Fetch and checkout an approved Git commit or release tag when safe."""

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
        resolved_ref = checkout_ref or required_version
        commands = (
            ["fetch", "--all", "--tags"],
            ["cat-file", "-e", f"{resolved_ref}^{{commit}}"],
            ["checkout", resolved_ref],
        )
        for command in commands:
            try:
                result = self._git_runner(command, cwd=source_path)
            except (OSError, RuntimeError, ValueError) as exc:
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
        requirements_failure = self._install_requirements(
            item,
            source_path=source_path,
        )
        if requirements_failure:
            return self._rollback_after_requirements_failure(
                item,
                source_path=source_path,
                failed_ref=resolved_ref,
                reason=requirements_failure,
            )
        return {
            "nodeId": node_id,
            "operation": "git_checkout",
            "command": list(commands[-1]),
            "returnCode": 0,
            "reason": "",
            "stdout": "",
            "stderr": "",
        }

    def _install_requirements(
        self,
        item: Mapping[str, Any],
        *,
        source_path: Path,
    ) -> str:
        """Install conventional Python requirements after an approved checkout."""

        requirements_path = source_path / "requirements.txt"
        if not requirements_path.is_file():
            return ""
        try:
            result = self._requirements_installer.install(requirements_path)
        except (OSError, RuntimeError, ValueError) as exc:
            return str(exc)
        if result.return_code == 0:
            return ""
        detail = (result.stderr or result.stdout).strip()[-2000:]
        node_id = normalize_metadata_string(item.get("nodeId")) or source_path.name
        return f"Could not install {node_id} requirements: {detail}"

    def _rollback_after_requirements_failure(
        self,
        item: Mapping[str, Any],
        *,
        source_path: Path,
        failed_ref: str,
        reason: str,
    ) -> dict[str, Any]:
        """Restore the prior Git commit when post-checkout requirements fail."""

        evidence = item.get("installedEvidence")
        previous_head = (
            normalize_metadata_string(evidence.get("gitHead"))
            if isinstance(evidence, Mapping)
            else ""
        )
        rollback_return_code = 1
        if previous_head:
            try:
                rollback = self._git_runner(
                    ["checkout", previous_head],
                    cwd=source_path,
                )
                rollback_return_code = int(getattr(rollback, "returncode", 0) or 0)
            except (OSError, RuntimeError, ValueError):
                rollback_return_code = 1
        return {
            "nodeId": normalize_metadata_string(item.get("nodeId")),
            "operation": "git_checkout",
            "command": ["checkout", failed_ref],
            "returnCode": 1,
            "reason": reason,
            "stdout": "",
            "stderr": "",
            "rollbackRef": previous_head,
            "rollbackSucceeded": rollback_return_code == 0,
        }


def _is_clean_semver_git_repair(item: Mapping[str, Any]) -> bool:
    """Return whether an installed clean Git checkout may follow its release tag."""

    required_version = normalize_metadata_string(item.get("requiredVersion"))
    if classify_version(required_version) != "semver":
        return False
    evidence = item.get("installedEvidence")
    if not isinstance(evidence, Mapping):
        return False
    return (
        normalize_metadata_string(evidence.get("sourceKind")) == "git"
        and evidence.get("dirty") is not True
        and bool(normalize_metadata_string(evidence.get("repositoryUrl")))
    )


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
