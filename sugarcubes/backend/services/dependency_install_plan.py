#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Project dependency requirement records into an installation plan."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .cube_dependency_manifest import normalize_requirement_key
from .cube_metadata import normalize_metadata_string


def installed_custom_nodes(custom_nodes_root: Path) -> set[str]:
    """Return installed target custom-node directory names."""

    if not custom_nodes_root.exists() or not custom_nodes_root.is_dir():
        return set()
    return {
        entry.name
        for entry in custom_nodes_root.iterdir()
        if entry.is_dir() and entry.name
    }


def build_dependency_install_plan(
    *,
    requirement_records: Sequence[Mapping[str, Any]],
    installed: set[str],
    version_plan: Sequence[Mapping[str, Any]] = (),
) -> list[dict[str, Any]]:
    """Collapse requirement records into one install plan per custom node."""

    installed_by_key = {normalize_requirement_key(name): name for name in installed}
    versions_by_key = {
        normalize_requirement_key(normalize_metadata_string(item.get("nodeId"))): item
        for item in version_plan
        if normalize_metadata_string(item.get("nodeId"))
    }
    by_node: dict[str, dict[str, Any]] = {}
    for record in requirement_records:
        node_id = normalize_metadata_string(record.get("node_id"))
        if not node_id:
            continue
        key = normalize_requirement_key(node_id)
        item = by_node.setdefault(
            key,
            {
                "nodeId": node_id,
                "displayName": normalize_metadata_string(record.get("display_name"))
                or node_id,
                "requiredVersion": "",
                "requiredVersionKind": "missing",
                "existingFolderName": "",
                "requiredByPacks": [],
                "requiredByCubeIds": [],
                "defaultBaseOnly": True,
                "confirmationRequired": False,
                "installable": True,
                "installed": False,
                "remediation": "",
            },
        )
        pack_ref = normalize_metadata_string(record.get("pack_ref"))
        cube_id = normalize_metadata_string(record.get("cube_id"))
        if pack_ref and pack_ref not in item["requiredByPacks"]:
            item["requiredByPacks"].append(pack_ref)
        if cube_id and cube_id not in item["requiredByCubeIds"]:
            item["requiredByCubeIds"].append(cube_id)
        if not bool(record.get("default_base_repo")):
            item["defaultBaseOnly"] = False
            item["confirmationRequired"] = True

    for key, item in by_node.items():
        version_item = versions_by_key.get(key)
        if version_item is not None:
            item["requiredVersion"] = normalize_metadata_string(
                version_item.get("requiredVersion")
            )
            item["requiredVersionKind"] = (
                normalize_metadata_string(version_item.get("requiredVersionKind"))
                or "missing"
            )
        existing_folder = installed_by_key.get(key, "")
        item["existingFolderName"] = existing_folder
        item["installed"] = bool(existing_folder)
        item["requiredByPacks"].sort(key=str.casefold)
        item["requiredByCubeIds"].sort(key=str.casefold)
        if not item["installed"] and not item["nodeId"]:
            item["installable"] = False
            item["remediation"] = (
                "Cube requirement does not include a Comfy Registry id."
            )
    return sorted(by_node.values(), key=lambda item: str(item["nodeId"]).casefold())
