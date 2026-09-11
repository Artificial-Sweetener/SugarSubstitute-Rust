#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Execute approved dependency version remediation."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .cube_metadata import normalize_metadata_string
from .dependency_acquisition import DependencyAcquirer
from .dependency_first_party_manifest import first_party_extension
from .dependency_version_types import GitRunner
from .dependency_versions import classify_version

_logger = logging.getLogger(__name__)


class DependencyVersionRepairExecutor:
    """Apply version repair through acquisition policy or a safe Git checkout."""

    def __init__(
        self,
        *,
        acquirer: DependencyAcquirer,
        git_runner: GitRunner,
    ) -> None:
        """Initialize version repair with explicit execution adapters."""

        self._acquirer = acquirer
        self._git_runner = git_runner

    def repair(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Repair one approved dependency version plan item."""

        status = normalize_metadata_string(item.get("status"))
        if status == "installed_commit_not_descendant":
            return self._checkout_required_git_ref(item)
        if _is_trusted_semver_git_repair(item):
            required_version = normalize_metadata_string(item.get("requiredVersion"))
            return self._checkout_required_git_ref(
                item,
                checkout_ref=f"v{required_version}",
            )
        return self._reinstall_versioned_node(item)

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
                _logger.warning(
                    "SugarCubes: Git version repair failed for %s",
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


def _is_trusted_semver_git_repair(item: Mapping[str, Any]) -> bool:
    """Return whether a clean official checkout may follow its exact release tag."""

    required_version = normalize_metadata_string(item.get("requiredVersion"))
    if classify_version(required_version) != "semver":
        return False
    evidence = item.get("installedEvidence")
    if not isinstance(evidence, Mapping):
        return False
    if normalize_metadata_string(evidence.get("sourceKind")) != "git":
        return False
    extension = first_party_extension(normalize_metadata_string(item.get("nodeId")))
    if extension is None:
        return False
    observed_repository = _normalized_repository(
        normalize_metadata_string(evidence.get("repositoryUrl"))
    )
    return observed_repository == _normalized_repository(extension.repository_url)


def _normalized_repository(value: str) -> str:
    """Normalize a repository URL for trusted-source identity comparison."""

    return value.strip().rstrip("/").removesuffix(".git").casefold()


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
