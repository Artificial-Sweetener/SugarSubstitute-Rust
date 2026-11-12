#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Extract dependency requirements from cube payloads."""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from typing import Any

from .dependency_version_types import CubeDependencyRequirement
from .dependency_versions import classify_version

_EXCLUDED_CUSTOM_NODE_SLUGS = frozenset({"websocket_image_save"})
_BUILT_IN_CUSTOM_NODE_IDS = frozenset({"comfy-core"})
_SUGARCUBES_CUSTOM_NODE_IDS = frozenset({"sugarcubes"})
_SUGARCUBES_MARKER_MODULES = frozenset({"nodes", "payloads"})


def extract_versioned_requirements(
    payload: Mapping[str, Any],
    *,
    cube_id: str,
    pack_ref: str,
    source_path: str,
    default_base_repo: bool,
) -> tuple[CubeDependencyRequirement, ...]:
    """Extract versioned dependency records from current and legacy cubes."""

    records: list[CubeDependencyRequirement] = []
    for node in _iter_workflow_nodes(payload):
        properties = node.get("properties")
        if not isinstance(properties, Mapping):
            continue
        node_id = _normalize_text(properties.get("cnr_id"))
        if not node_id:
            continue
        required_version = _normalize_text(properties.get("ver"))
        records.append(
            CubeDependencyRequirement(
                node_id=node_id,
                required_version=required_version,
                version_kind=classify_version(required_version),
                cube_id=cube_id,
                pack_ref=pack_ref,
                node_name=_normalize_text(properties.get("Node name for S&R"))
                or _normalize_text(node.get("type"))
                or node_id,
                class_type=_normalize_text(node.get("type")),
                source_path=source_path,
                default_base_repo=default_base_repo,
            )
        )

    existing_keys = {normalize_requirement_key(record.node_id) for record in records}
    for slug in _iter_custom_node_slugs(payload):
        if not is_external_custom_node_requirement(slug):
            continue
        if normalize_requirement_key(slug) in existing_keys:
            continue
        records.append(
            CubeDependencyRequirement(
                node_id=slug,
                required_version="",
                version_kind="missing",
                cube_id=cube_id,
                pack_ref=pack_ref,
                node_name=slug,
                class_type="",
                source_path=source_path,
                default_base_repo=default_base_repo,
            )
        )
    return tuple(records)


def comfy_runtime_readiness(
    requirements: Sequence[CubeDependencyRequirement],
) -> dict[str, Any]:
    """Return Comfy runtime requirements represented by `comfy-core` facts."""

    core_requirements = [
        requirement
        for requirement in requirements
        if normalize_requirement_key(requirement.node_id) == "comfy-core"
    ]
    if not core_requirements:
        return {
            "schemaVersion": 1,
            "requiredVersion": "",
            "requiredVersionKind": "missing",
            "installedVersion": "",
            "status": "satisfied",
            "requirements": [],
        }
    strongest = _strongest_semver_requirement(core_requirements)
    return {
        "schemaVersion": 1,
        "requiredVersion": strongest,
        "requiredVersionKind": classify_version(strongest),
        "installedVersion": "",
        "status": "installed_version_unknown",
        "requirements": [requirement.to_payload() for requirement in core_requirements],
        "remediation": "Comfy runtime version could not be read from the active host.",
    }


def is_external_custom_node_requirement(value: str) -> bool:
    """Return whether a node id is an installable custom-node requirement."""

    normalized = normalize_requirement_key(value)
    if not normalized:
        return False
    return (
        normalized not in _BUILT_IN_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_MARKER_MODULES
    )


def normalize_requirement_key(value: str) -> str:
    """Return a stable key for comparing registry ids and folder names."""

    return re.sub(r"[-_.]+", "-", value.strip().casefold())


def _strongest_semver_requirement(
    requirements: Sequence[CubeDependencyRequirement],
) -> str:
    """Return the highest semver-looking requirement from runtime facts."""

    semver_versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.version_kind == "semver"
    ]
    if semver_versions:
        return max(semver_versions, key=_semver_key)
    versions = [
        requirement.required_version
        for requirement in requirements
        if requirement.required_version
    ]
    return versions[0] if versions else ""


def _iter_workflow_nodes(value: Any) -> tuple[Mapping[str, Any], ...]:
    """Return workflow node dictionaries from nested cube payload shapes."""

    nodes: list[Mapping[str, Any]] = []
    if isinstance(value, Mapping):
        properties = value.get("properties")
        if isinstance(properties, Mapping) and "cnr_id" in properties:
            nodes.append(value)
        for child in value.values():
            nodes.extend(_iter_workflow_nodes(child))
    elif isinstance(value, list):
        for child in value:
            nodes.extend(_iter_workflow_nodes(child))
    return tuple(nodes)


def _iter_custom_node_slugs(payload: Mapping[str, Any]) -> tuple[str, ...]:
    """Return fallback custom-node module slugs from supported cube shapes."""

    slugs: set[str] = set()
    for module_name in _iter_python_modules(payload):
        if not module_name.startswith("custom_nodes."):
            continue
        slug = module_name.split(".", 1)[1].strip()
        if slug and slug not in _EXCLUDED_CUSTOM_NODE_SLUGS:
            slugs.add(slug)
    return tuple(sorted(slugs))


def _iter_python_modules(payload: Mapping[str, Any]) -> tuple[str, ...]:
    """Return python module references from supported cube containers."""

    modules: list[str] = []
    definitions = payload.get("definitions")
    if isinstance(definitions, Mapping):
        modules.extend(_definition_modules(definitions))
    implementation = payload.get("implementation")
    if isinstance(implementation, Mapping):
        definitions = implementation.get("definitions")
        if isinstance(definitions, Mapping):
            modules.extend(_definition_modules(definitions))
    return tuple(modules)


def _definition_modules(definitions: Mapping[str, Any]) -> list[str]:
    """Return module strings from a definitions mapping."""

    modules: list[str] = []
    for spec in definitions.values():
        if not isinstance(spec, Mapping):
            continue
        module_name = spec.get("python_module")
        if isinstance(module_name, str) and module_name.strip():
            modules.append(module_name.strip())
    return modules


def _semver_key(value: str) -> tuple[int, int, int, int, str]:
    """Return a conservative sortable key for semver-like strings."""

    main, _, suffix = value.partition("-")
    numeric = [int(part) for part in main.split(".") if part.isdigit()]
    padded = [*numeric, 0, 0, 0, 0][:4]
    return (padded[0], padded[1], padded[2], padded[3], suffix)


def _normalize_text(value: object) -> str:
    """Return a stripped string or an empty string."""

    return value.strip() if isinstance(value, str) else ""
