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
"""Project validated Cube artifacts into host-neutral picker descriptors."""

from __future__ import annotations

import logging
import hashlib
from collections.abc import Callable, Mapping, Sequence
from typing import Any, TypedDict

from ...importer import CubeImportError
from ..responses import BackendError
from .cube_metadata import normalize_metadata_string

_logger = logging.getLogger(__name__)


class CubePickerBoundary(TypedDict):
    """Describe one public Cube boundary without internal implementation endpoints."""

    id: str
    name: str
    label: str
    type: str


class CubePickerDescriptor(TypedDict):
    """Describe one Cube for discovery by a host node picker."""

    key: str
    cubeId: str
    version: str
    displayName: str
    description: str
    searchTerms: list[str]
    targetModel: str
    supportedModels: list[str]
    requiredCustomNodes: list[str]
    source: dict[str, Any]
    inputs: list[CubePickerBoundary]
    outputs: list[CubePickerBoundary]


class CubePickerCatalogError(TypedDict):
    """Report one catalog row that cannot safely become a picker definition."""

    cubeId: str
    message: str


CatalogProvider = Callable[[], Mapping[str, Any]]
CubeLoader = Callable[[str], Mapping[str, Any]]


class CubePickerCatalogService:
    """Own the lightweight catalog contract consumed by node-picker adapters."""

    def __init__(
        self,
        *,
        list_catalog: CatalogProvider,
        load_cube: CubeLoader,
    ) -> None:
        """Bind catalog identity and canonical prepared-import providers."""

        self._list_catalog = list_catalog
        self._load_cube = load_cube

    def list_picker_catalog(self) -> dict[str, Any]:
        """Return valid picker descriptors while isolating invalid artifacts."""

        catalog = self._list_catalog()
        revision = normalize_metadata_string(catalog.get("catalogRevision"))
        descriptors: list[CubePickerDescriptor] = []
        errors: list[CubePickerCatalogError] = []
        for entry in _mapping_sequence(catalog.get("cubes")):
            cube_id = normalize_metadata_string(entry.get("cubeId"))
            if not cube_id:
                errors.append(
                    {"cubeId": "", "message": "Catalog entry is missing Cube identity"}
                )
                continue
            try:
                descriptors.append(self._project_descriptor(entry, cube_id))
            except (BackendError, CubeImportError, TypeError, ValueError) as error:
                message = _error_message(error)
                _logger.warning(
                    "SugarCubes picker catalog omitted an invalid Cube",
                    extra={"cube_id": cube_id, "reason": message},
                )
                errors.append({"cubeId": cube_id, "message": message})
            except (OSError, RuntimeError):
                _logger.exception(
                    "SugarCubes picker catalog failed to project Cube",
                    extra={"cube_id": cube_id},
                )
                errors.append(
                    {"cubeId": cube_id, "message": "Cube picker projection failed"}
                )
        return {
            "schemaVersion": 1,
            "catalogRevision": revision,
            "entries": descriptors,
            "errors": errors,
        }

    def _project_descriptor(
        self,
        entry: Mapping[str, Any],
        cube_id: str,
    ) -> CubePickerDescriptor:
        """Build one descriptor from catalog metadata and canonical boundaries."""

        prepared = self._load_cube(cube_id)
        cube_value = prepared.get("cube")
        cube = cube_value if isinstance(cube_value, Mapping) else {}
        metadata_value = cube.get("metadata")
        metadata = metadata_value if isinstance(metadata_value, Mapping) else {}
        source_value = entry.get("source")
        source = dict(source_value) if isinstance(source_value, Mapping) else {}
        display_name = (
            normalize_metadata_string(cube.get("default_alias"))
            or normalize_metadata_string(entry.get("displayName"))
            or cube_id
        )
        description = normalize_metadata_string(cube.get("description")) or (
            normalize_metadata_string(entry.get("description"))
        )
        target_model = normalize_metadata_string(entry.get("targetModel"))
        supported_models = _string_sequence(entry.get("supportedModels"))
        required_nodes = _string_sequence(entry.get("requiredCustomNodes"))
        inputs, outputs = _project_boundaries(prepared.get("boundaries"))
        search_terms = _dedupe_terms(
            [
                cube_id,
                display_name,
                *_string_sequence(metadata.get("tags")),
                normalize_metadata_string(metadata.get("author")),
                normalize_metadata_string(source.get("repoRef")),
                normalize_metadata_string(source.get("path")),
                target_model,
                *supported_models,
            ]
        )
        return {
            "key": hashlib.sha256(cube_id.encode("utf-8")).hexdigest(),
            "cubeId": cube_id,
            "version": normalize_metadata_string(cube.get("version"))
            or normalize_metadata_string(entry.get("version")),
            "displayName": display_name,
            "description": description,
            "searchTerms": search_terms,
            "targetModel": target_model,
            "supportedModels": supported_models,
            "requiredCustomNodes": required_nodes,
            "source": source,
            "inputs": inputs,
            "outputs": outputs,
        }


def _project_boundaries(
    value: object,
) -> tuple[list[CubePickerBoundary], list[CubePickerBoundary]]:
    """Strip internal endpoints from one canonical boundary projection."""

    if not isinstance(value, Mapping):
        raise ValueError("Prepared Cube is missing canonical boundaries")
    return (
        [_project_boundary(entry) for entry in _mapping_sequence(value.get("inputs"))],
        [_project_boundary(entry) for entry in _mapping_sequence(value.get("outputs"))],
    )


def _project_boundary(value: Mapping[str, Any]) -> CubePickerBoundary:
    """Validate one public boundary identity before advertising it."""

    boundary_id = normalize_metadata_string(value.get("id"))
    name = normalize_metadata_string(value.get("name"))
    label = normalize_metadata_string(value.get("label"))
    boundary_type = normalize_metadata_string(value.get("type"))
    missing = [
        field_name
        for field_name, field_value in (
            ("id", boundary_id),
            ("name", name),
            ("label", label),
            ("type", boundary_type),
        )
        if not field_value
    ]
    if missing:
        raise ValueError(
            f"Canonical boundary is missing required fields: {', '.join(missing)}"
        )
    return {
        "id": boundary_id,
        "name": name,
        "label": label,
        "type": boundary_type,
    }


def _mapping_sequence(value: object) -> list[Mapping[str, Any]]:
    """Narrow a dynamic sequence to mapping entries."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [entry for entry in value if isinstance(entry, Mapping)]


def _string_sequence(value: object) -> list[str]:
    """Normalize one dynamic sequence to non-empty strings."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [text for item in value if (text := normalize_metadata_string(item))]


def _dedupe_terms(values: Sequence[str]) -> list[str]:
    """Deduplicate discovery terms case-insensitively while preserving display text."""

    result: list[str] = []
    seen: set[str] = set()
    for value in values:
        text = normalize_metadata_string(value)
        key = text.casefold()
        if not text or key in seen:
            continue
        seen.add(key)
        result.append(text)
    return result


def _error_message(error: Exception) -> str:
    """Read one safe actionable error message without exposing local paths."""

    if isinstance(error, BackendError):
        return error.message
    if isinstance(error, CubeImportError):
        return error.message
    text = str(error).strip()
    return text or "Cube picker projection failed"
