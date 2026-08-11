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

from ..responses import BackendError
from .cube_metadata import normalize_metadata_string
from .dependency_cli import ComfyCliAdapter
from .dependency_version_types import GitRunner

_logger = logging.getLogger(__name__)


class DependencyVersionRepairExecutor:
    """Apply version repair through Comfy CLI or an installed Git checkout."""

    def __init__(
        self,
        *,
        workspace_path: Path,
        cli_adapter: ComfyCliAdapter,
        git_runner: GitRunner,
    ) -> None:
        """Initialize version repair with explicit execution adapters."""

        self._workspace_path = workspace_path
        self._cli_adapter = cli_adapter
        self._git_runner = git_runner

    def repair(self, item: Mapping[str, Any]) -> dict[str, Any]:
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
        """Fetch and checkout the approved required Git commit when safe."""

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
        commands = (
            ["fetch", "--all", "--tags"],
            ["cat-file", "-e", f"{required_version}^{{commit}}"],
            ["checkout", required_version],
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
