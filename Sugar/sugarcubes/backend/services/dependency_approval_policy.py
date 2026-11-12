#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Select dependency repair work under explicit approval policy."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any, Literal

from .cube_metadata import normalize_metadata_string

DependencyApprovalPolicy = Literal[
    "silent_baseline_only",
    "approved_node_ids",
    "approve_all",
]


def approval_policy_from_payload(
    dependency_policy: Mapping[str, Any],
) -> DependencyApprovalPolicy:
    """Return repair approval policy from a sync-and-check request."""

    if bool(dependency_policy.get("approveAll")):
        return "approve_all"
    if bool(dependency_policy.get("baselineOnly")):
        return "silent_baseline_only"
    return "approved_node_ids"


def approved_node_ids(dependency_policy: Mapping[str, Any]) -> tuple[str, ...]:
    """Return normalized approved node ids from a request."""

    values = dependency_policy.get("approvedNodeIds")
    if not isinstance(values, list):
        return ()
    return tuple(
        normalize_metadata_string(node_id)
        for node_id in values
        if normalize_metadata_string(node_id)
    )


def select_install_items(
    install_plan: object,
    *,
    approval_policy: DependencyApprovalPolicy,
    approved_node_ids: Sequence[str],
) -> list[dict[str, Any]]:
    """Return install items allowed by the requested approval policy."""

    approved = {normalize_metadata_string(node_id) for node_id in approved_node_ids}
    selected: list[dict[str, Any]] = []
    for item in iter_plan_items(install_plan):
        if item.get("installed") is True or item.get("installable") is not True:
            continue
        node_id = normalize_metadata_string(item.get("nodeId"))
        confirmation_required = bool(item.get("confirmationRequired"))
        if approval_policy == "approve_all":
            selected.append(item)
        elif approval_policy == "silent_baseline_only" and not confirmation_required:
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


def skipped_install_items(
    install_plan: object,
    selected_items: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Return missing items that were not selected for installation."""

    selected = {
        normalize_metadata_string(item.get("nodeId")) for item in selected_items
    }
    return [
        item
        for item in iter_plan_items(install_plan)
        if item.get("installed") is not True
        and normalize_metadata_string(item.get("nodeId")) not in selected
    ]


def select_version_items(
    version_plan: object,
    *,
    approval_policy: DependencyApprovalPolicy,
    approved_node_ids: Sequence[str],
) -> list[dict[str, Any]]:
    """Return approved version plan items that can be repaired."""

    approved = {normalize_metadata_string(node_id) for node_id in approved_node_ids}
    selected: list[dict[str, Any]] = []
    for item in iter_plan_items(version_plan):
        if item.get("status") in {"satisfied", "missing"}:
            continue
        if item.get("repairable") is not True:
            continue
        node_id = normalize_metadata_string(item.get("nodeId"))
        confirmation_required = version_item_confirmation_required(item)
        if approval_policy == "approve_all":
            selected.append(item)
        elif approval_policy == "silent_baseline_only" and not confirmation_required:
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


def skipped_version_items(
    version_plan: object,
    selected_items: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Return repairable version items that were not approved."""

    selected = {
        normalize_metadata_string(item.get("nodeId")) for item in selected_items
    }
    return [
        item
        for item in iter_plan_items(version_plan)
        if item.get("status") not in {"satisfied", "missing"}
        and normalize_metadata_string(item.get("nodeId")) not in selected
        and item.get("repairable") is True
    ]


def blocked_version_items(version_plan: object) -> list[dict[str, Any]]:
    """Return unsatisfied version items that automatic repair must preserve."""

    return [
        item
        for item in iter_plan_items(version_plan)
        if item.get("status") in {"blocked", "version_conflict"}
        and item.get("repairable") is not True
    ]


def iter_plan_items(plan: object) -> list[dict[str, Any]]:
    """Coerce a dependency-plan payload into item dictionaries."""

    if not isinstance(plan, list):
        return []
    return [dict(item) for item in plan if isinstance(item, Mapping)]


def version_item_confirmation_required(item: Mapping[str, Any]) -> bool:
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
