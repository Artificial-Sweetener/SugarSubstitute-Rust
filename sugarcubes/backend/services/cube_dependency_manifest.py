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
"""Extract installable custom-node requirements from cube documents."""

from __future__ import annotations

import re
from typing import Any, Mapping, Sequence

_EXCLUDED_CUSTOM_NODE_SLUGS = frozenset({"websocket_image_save"})
_BUILT_IN_CUSTOM_NODE_IDS = frozenset({"comfy-core"})
_SUGARCUBES_CUSTOM_NODE_IDS = frozenset({"sugarcubes"})
_SUGARCUBES_MARKER_MODULES = frozenset({"nodes", "payloads"})


def iter_custom_node_slugs(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return custom-node slugs referenced by supported cube definition shapes."""

    slugs: set[str] = set()
    for module_name in _iter_python_modules(payload):
        if not module_name.startswith("custom_nodes."):
            continue
        slug = module_name.split(".", 1)[1].strip()
        if slug and slug not in _EXCLUDED_CUSTOM_NODE_SLUGS:
            slugs.add(slug)
    return tuple(sorted(slugs))


def iter_custom_node_requirement_ids(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return install-oriented custom-node ids required by a cube payload."""

    cnr_ids = {
        requirement
        for requirement in _iter_cnr_ids(payload)
        if _is_external_custom_node_requirement(requirement)
    }
    module_slugs = {
        slug
        for slug in iter_custom_node_slugs(payload)
        if _is_external_custom_node_requirement(slug)
    }
    normalized_cnr_ids = {
        normalize_requirement_key(requirement) for requirement in cnr_ids
    }
    requirements = set(cnr_ids)
    for slug in module_slugs:
        if normalize_requirement_key(slug) not in normalized_cnr_ids:
            requirements.add(slug)
    return tuple(sorted(requirements, key=str.casefold))


def _iter_cnr_ids(value: Any) -> Sequence[str]:
    """Return all Comfy Registry ids embedded in current cube payload shapes."""

    found: list[str] = []
    if isinstance(value, Mapping):
        cnr_id = value.get("cnr_id")
        if isinstance(cnr_id, str) and cnr_id.strip():
            found.append(cnr_id.strip())
        for child in value.values():
            found.extend(_iter_cnr_ids(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(_iter_cnr_ids(child))
    return tuple(found)


def _is_external_custom_node_requirement(value: str) -> bool:
    """Return whether a requirement should be handled as an installable node."""

    normalized = normalize_requirement_key(value)
    if not normalized:
        return False
    return (
        normalized not in _BUILT_IN_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_CUSTOM_NODE_IDS
        and normalized not in _SUGARCUBES_MARKER_MODULES
    )


def normalize_requirement_key(value: str) -> str:
    """Return a stable comparison key for custom-node ids and folder names."""

    return re.sub(r"[-_.]+", "-", value.strip().casefold())


def _iter_python_modules(payload: Mapping[str, Any]) -> Sequence[str]:
    """Return python module references from current and legacy cube payloads."""

    modules: list[str] = []
    definitions = payload.get("definitions")
    if isinstance(definitions, Mapping):
        for spec in definitions.values():
            module_name = spec.get("python_module") if isinstance(spec, Mapping) else None
            if isinstance(module_name, str):
                modules.append(module_name)
        return tuple(modules)

    implementation = payload.get("implementation")
    implementation_definitions = (
        implementation.get("definitions")
        if isinstance(implementation, Mapping)
        else None
    )
    if isinstance(implementation_definitions, Mapping):
        for spec in implementation_definitions.values():
            module_name = spec.get("python_module") if isinstance(spec, Mapping) else None
            if isinstance(module_name, str):
                modules.append(module_name)
        return tuple(modules)

    for value in payload.values():
        if not isinstance(value, Mapping):
            continue
        module_name = value.get("python_module")
        if isinstance(module_name, str):
            modules.append(module_name)
    return tuple(modules)
