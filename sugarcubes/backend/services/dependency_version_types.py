#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Define dependency-version facts shared by policy and environment adapters."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Literal

VersionKind = Literal["semver", "git_sha", "unknown", "missing"]
DependencyStatus = Literal[
    "satisfied",
    "missing",
    "installed_version_unknown",
    "installed_version_too_old",
    "installed_commit_not_descendant",
    "version_conflict",
    "not_comparable",
    "not_repairable",
    "blocked",
]
GitRunner = Callable[..., object]
GitContains = Callable[[str, str, str], bool]


@dataclass(frozen=True)
class CubeDependencyRequirement:
    """Describe one dependency fact contributed by one cube node or module."""

    node_id: str
    required_version: str
    version_kind: VersionKind
    cube_id: str
    pack_ref: str
    node_name: str
    class_type: str
    source_path: str
    default_base_repo: bool

    def to_payload(self) -> dict[str, Any]:
        """Return this requirement as a JSON-safe payload."""

        return {
            "nodeId": self.node_id,
            "requiredVersion": self.required_version,
            "requiredVersionKind": self.version_kind,
            "cubeId": self.cube_id,
            "packRef": self.pack_ref,
            "nodeName": self.node_name,
            "classType": self.class_type,
            "sourcePath": self.source_path,
            "defaultBaseRepo": self.default_base_repo,
        }


@dataclass(frozen=True)
class InstalledDependency:
    """Describe installed evidence for one custom-node folder."""

    folder_name: str
    source_path: str
    installed_version: str
    version_kind: VersionKind
    source_kind: str
    repository_url: str
    dirty: bool

    def to_payload(self) -> dict[str, Any]:
        """Return this installed evidence as a JSON-safe payload."""

        return {
            "folderName": self.folder_name,
            "sourcePath": self.source_path,
            "installedVersion": self.installed_version,
            "installedVersionKind": self.version_kind,
            "sourceKind": self.source_kind,
            "repositoryUrl": self.repository_url,
            "dirty": self.dirty,
        }
