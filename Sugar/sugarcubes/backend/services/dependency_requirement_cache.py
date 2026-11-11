#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Persist dependency requirement inventories across Comfy processes."""

from __future__ import annotations

import json
import logging
import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .cube_metadata import normalize_metadata_string
from .dependency_version_types import (
    CubeDependencyRequirement,
    RequirementOrigin,
    VersionRequirementPolicy,
)
from .dependency_versions import classify_version

_logger = logging.getLogger(__name__)
_CACHE_SCHEMA_VERSION = 2
_CACHE_FILENAME = "dependency-requirements.json"


@dataclass(frozen=True)
class DependencyRequirementSet:
    """Collect discovered dependency requirements and their catalog revision."""

    records: list[dict[str, Any]]
    version_requirements: tuple[CubeDependencyRequirement, ...]
    catalog_revision: str


class DependencyRequirementCache:
    """Read and atomically write dependency requirement inventories."""

    def __init__(self, extension_root: Path) -> None:
        """Initialize the cache under one SugarCubes extension root."""

        self._extension_root = extension_root

    def load(self, source_signature: str) -> DependencyRequirementSet | None:
        """Return durable requirements when source facts still match."""

        try:
            raw = json.loads(self._path().read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None
        except (OSError, json.JSONDecodeError, UnicodeDecodeError, TypeError):
            _logger.warning(
                "SugarCubes: failed to read dependency requirement cache",
                exc_info=True,
            )
            return None
        if not isinstance(raw, Mapping):
            return None
        if raw.get("schemaVersion") != _CACHE_SCHEMA_VERSION:
            return None
        if raw.get("sourceSignature") != source_signature:
            return None
        requirement_records = raw.get("requirementRecords")
        version_requirements = raw.get("versionRequirements")
        if not isinstance(requirement_records, list) or not isinstance(
            version_requirements, list
        ):
            return None
        try:
            return DependencyRequirementSet(
                records=[
                    dict(record)
                    for record in requirement_records
                    if isinstance(record, Mapping)
                ],
                version_requirements=tuple(
                    _requirement_from_payload(record)
                    for record in version_requirements
                    if isinstance(record, Mapping)
                ),
                catalog_revision=normalize_metadata_string(raw.get("catalogRevision")),
            )
        except (TypeError, ValueError):
            _logger.warning(
                "SugarCubes: dependency requirement cache payload is invalid",
                exc_info=True,
            )
            return None

    def store(
        self,
        *,
        source_signature: str,
        result: DependencyRequirementSet,
    ) -> None:
        """Persist dependency requirements for reuse by the next process."""

        cache_path = self._path()
        payload = {
            "schemaVersion": _CACHE_SCHEMA_VERSION,
            "sourceSignature": source_signature,
            "catalogRevision": result.catalog_revision,
            "requirementRecords": [dict(record) for record in result.records],
            "versionRequirements": [
                requirement.to_payload() for requirement in result.version_requirements
            ],
        }
        try:
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            temp_path = cache_path.with_name(f"{cache_path.name}.{os.getpid()}.tmp")
            temp_path.write_text(
                json.dumps(payload, sort_keys=True, separators=(",", ":")),
                encoding="utf-8",
            )
            temp_path.replace(cache_path)
        except OSError:
            _logger.warning(
                "SugarCubes: failed to write dependency requirement cache",
                exc_info=True,
            )

    def _path(self) -> Path:
        """Return the durable dependency-requirement cache location."""

        return self._extension_root / ".sugarcubes" / "cache" / _CACHE_FILENAME


def _requirement_from_payload(payload: Mapping[str, Any]) -> CubeDependencyRequirement:
    """Rehydrate one cached versioned dependency requirement."""

    required_version = normalize_metadata_string(payload.get("requiredVersion"))
    return CubeDependencyRequirement(
        node_id=normalize_metadata_string(payload.get("nodeId")),
        required_version=required_version,
        version_kind=classify_version(required_version),
        cube_id=normalize_metadata_string(payload.get("cubeId")),
        pack_ref=normalize_metadata_string(payload.get("packRef")),
        node_name=normalize_metadata_string(payload.get("nodeName")),
        class_type=normalize_metadata_string(payload.get("classType")),
        source_path=normalize_metadata_string(payload.get("sourcePath")),
        default_base_repo=bool(payload.get("defaultBaseRepo")),
        version_policy=_version_policy(payload.get("requiredVersionPolicy")),
        requirement_origin=_requirement_origin(payload.get("requirementOrigin")),
        implied_by_node_id=normalize_metadata_string(payload.get("impliedByNodeId")),
    )


def _version_policy(value: object) -> VersionRequirementPolicy:
    """Return a supported cached version policy or the direct default."""

    return "exact" if value == "exact" else "minimum"


def _requirement_origin(value: object) -> RequirementOrigin:
    """Return a supported cached requirement origin or the direct default."""

    return "implied" if value == "implied" else "direct"
