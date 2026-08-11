#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Project dependency maintenance outcomes into user-facing diagnostics."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any, Literal

from .cube_metadata import normalize_metadata_string
from .dependency_approval_policy import (
    iter_plan_items,
    version_item_confirmation_required,
)

DiagnosticSeverity = Literal["info", "warning", "error"]


def diagnostics_from_sync_errors(
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
                    "SugarCubes could not update Base-Cubes and is using the local checkout."
                    if "Base-Cubes" in repo_ref
                    else "SugarCubes could not update one cube pack and is using local data."
                ),
                details=error,
            )
        )
    return diagnostics


def diagnostics_from_repair_result(
    repair_result: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Return startup diagnostics from dependency repair outcomes."""

    diagnostics: list[dict[str, Any]] = []
    for item in iter_plan_items(repair_result.get("failedNodes")):
        node_id = normalize_metadata_string(item.get("nodeId"))
        diagnostics.append(
            _diagnostic(
                code="sugarcubes_dependency_install_failed",
                severity="error",
                title="SugarCubes dependency install failed",
                message=(
                    f"{node_id} could not be installed automatically. Cube workflows that require it may fail until it is repaired."
                    if node_id
                    else "A cube dependency could not be installed automatically."
                ),
                details=item,
            )
        )
    for item in iter_plan_items(repair_result.get("failedVersionItems")):
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
    for item in iter_plan_items(repair_result.get("skippedNodes")):
        if bool(item.get("confirmationRequired")):
            diagnostics.append(_approval_required_diagnostic(item))
    for item in iter_plan_items(repair_result.get("skippedVersionItems")):
        if version_item_confirmation_required(item):
            diagnostics.append(_approval_required_diagnostic(item))
    return diagnostics


def _approval_required_diagnostic(item: Mapping[str, Any]) -> dict[str, Any]:
    """Return a diagnostic for work that cannot be repaired silently."""

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


def _json_safe_details(details: Mapping[str, Any]) -> dict[str, Any]:
    """Return a compact JSON-safe diagnostic details mapping."""

    safe: dict[str, Any] = {}
    for key, value in details.items():
        if isinstance(value, Mapping):
            safe[str(key)] = _json_safe_details(value)
        elif isinstance(value, list):
            safe[str(key)] = [
                _json_safe_details(item)
                if isinstance(item, Mapping)
                else _json_safe_scalar(item)
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
