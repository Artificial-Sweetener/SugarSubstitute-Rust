#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Acquire custom nodes through Registry-first authoritative source policy."""

from __future__ import annotations

import logging
import subprocess
import urllib.error
import zipfile
from collections.abc import Mapping
from time import perf_counter
from typing import Any

from ...instrumentation.logger import log_diagnostic
from .cube_metadata import normalize_metadata_string
from .dependency_registry_source import RegistrySourceResolver
from .dependency_source_archive import TrustedSourceArchiveInstaller
from .dependency_source_git import RegistrySourceGitInstaller
from .dependency_versions import classify_version

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes dependency acquisition diagnostic"


class DependencyAcquirer:
    """Coordinate Registry acquisition with its authoritative source fallback."""

    def __init__(
        self,
        *,
        source_installer: TrustedSourceArchiveInstaller,
        source_resolver: RegistrySourceResolver,
        git_installer: RegistrySourceGitInstaller,
    ) -> None:
        """Initialize acquisition through explicit external-system adapters."""

        self._source_installer = source_installer
        self._source_resolver = source_resolver
        self._git_installer = git_installer

    def acquire(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Acquire one approved dependency without trusting cube-authored URLs."""

        started_at = perf_counter()
        payload = self._acquire(item)
        node_id = normalize_metadata_string(item.get("nodeId"))
        required_version = normalize_metadata_string(item.get("requiredVersion"))
        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_dependency_acquisition_timing",
            {
                "node_id": node_id,
                "required_version": required_version,
                "required_version_kind": normalize_metadata_string(
                    item.get("requiredVersionKind")
                ),
                "required_version_policy": normalize_metadata_string(
                    item.get("requiredVersionPolicy")
                ),
                "acquisition_source": normalize_metadata_string(
                    payload.get("acquisitionSource")
                ),
                "operation": normalize_metadata_string(payload.get("operation")),
                "outcome": ("success" if payload.get("returnCode") == 0 else "failure"),
                "total_duration_ms": round(
                    (perf_counter() - started_at) * 1000,
                    3,
                ),
            },
        )
        return payload

    def _acquire(self, item: Mapping[str, Any]) -> dict[str, Any]:
        """Execute one validated targeted acquisition."""

        node_id = normalize_metadata_string(item.get("nodeId"))
        required_version = normalize_metadata_string(item.get("requiredVersion"))
        try:
            extension = self._source_resolver.resolve(node_id, required_version)
            unsafe_reason = _unsafe_existing_source_reason(
                item.get("installedEvidence"),
                expected_repository=extension.repository_url,
            )
            if unsafe_reason:
                return {
                    **_failed_result(
                        node_id=node_id,
                        required_version=required_version,
                        operation="registry_source_install",
                        reason=unsafe_reason,
                    )
                }
            if classify_version(required_version) == "git_sha":
                git_result = self._git_installer.install(
                    extension=extension,
                    git_ref=required_version,
                )
                return {
                    "nodeId": node_id,
                    "requestedVersion": required_version,
                    "operation": "registry_source_git_install",
                    "acquisitionSource": "github_commit",
                    "installedVersion": git_result.git_ref,
                    "repositoryUrl": git_result.repository_url,
                    "targetPath": str(git_result.target_path),
                    "returnCode": 0,
                    "stdout": "",
                    "stderr": "",
                    "reason": "",
                }
            source_result = self._source_installer.install(
                extension=extension,
                version=required_version,
            )
        except (
            OSError,
            RuntimeError,
            ValueError,
            urllib.error.URLError,
            zipfile.BadZipFile,
            subprocess.SubprocessError,
        ) as exc:
            _logger.warning(
                "SugarCubes: Registry source fallback failed for %s@%s",
                node_id,
                required_version,
                exc_info=True,
            )
            return {
                **_failed_result(
                    node_id=node_id,
                    required_version=required_version,
                    operation="registry_source_install",
                    reason=str(exc).strip() or type(exc).__name__,
                )
            }
        return {
            "nodeId": node_id,
            "requestedVersion": required_version,
            "operation": "registry_source_install",
            "acquisitionSource": (
                "registry_artifact" if extension.package_url else "github_source"
            ),
            "installedVersion": source_result.version,
            "archiveUrl": source_result.archive_url,
            "repositoryUrl": source_result.repository_url,
            "targetPath": str(source_result.target_path),
            "trackedFileCount": source_result.tracked_file_count,
            "returnCode": 0,
            "stdout": "",
            "stderr": "",
            "reason": "",
        }


def _unsafe_existing_source_reason(
    evidence_value: object,
    *,
    expected_repository: str,
) -> str:
    """Reject fallback mutation when installed ownership cannot be proven."""

    if not isinstance(evidence_value, Mapping):
        return ""
    source_kind = normalize_metadata_string(evidence_value.get("sourceKind"))
    if source_kind == "git":
        return "installed_source_is_git_checkout"
    if source_kind != "tracking":
        return "installed_source_not_registry_owned"
    observed_repository = normalize_metadata_string(evidence_value.get("repositoryUrl"))
    if expected_repository and _normalized_repository(
        observed_repository
    ) != _normalized_repository(expected_repository):
        return "installed_repository_does_not_match_trusted_source"
    return ""


def _failed_result(
    *,
    node_id: str,
    required_version: str,
    operation: str,
    reason: str,
) -> dict[str, Any]:
    """Return one consistent acquisition failure payload."""

    return {
        "nodeId": node_id,
        "requestedVersion": required_version,
        "operation": operation,
        "returnCode": 1,
        "reason": reason,
        "stdout": "",
        "stderr": "",
    }


def _normalized_repository(value: str) -> str:
    """Normalize official repository identity for comparison."""

    return value.strip().rstrip("/").removesuffix(".git").casefold()


__all__ = ["DependencyAcquirer"]
