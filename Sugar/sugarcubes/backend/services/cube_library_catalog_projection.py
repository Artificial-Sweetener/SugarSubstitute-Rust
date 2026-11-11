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
"""Project Cube summaries and tracked repositories into catalog API records."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Optional

from .cube_dependency_manifest import iter_custom_node_slugs
from .cube_file_io import (
    list_cube_files,
    read_cube_payload_with_hash as read_cube_payload_with_hash,
)
from .cube_library_diagnostics import log_cube_library_diagnostic
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .cube_metadata import normalize_metadata_string
from .tracked_repo_service import TrackedRepoService


class CubeLibraryCatalogProjection:
    """Own API-safe Cube catalog and pack record projection."""

    def __init__(
        self,
        *,
        tracked_repo_service: TrackedRepoService,
        sources: CubeLibrarySourceResolver,
    ) -> None:
        """Initialize catalog projection with source and repository owners."""

        self.tracked_repo_service = tracked_repo_service
        self.sources = sources

    def catalog_entry_for_summary(self, summary: Mapping[str, Any]) -> dict[str, Any]:
        """Convert a browser cube summary into a backend catalog entry."""

        cube_id = normalize_metadata_string(summary.get("cube_id"))
        payload, _, content_hash = self.summary_payload_with_hash(summary)
        icon = summary.get("icon") if isinstance(summary.get("icon"), Mapping) else None
        entry: dict[str, Any] = {
            "cubeId": cube_id,
            "displayName": normalize_metadata_string(summary.get("display_name"))
            or normalize_metadata_string(summary.get("name")),
            "version": normalize_metadata_string(summary.get("version")),
            "description": normalize_metadata_string(summary.get("description")),
            "targetModel": normalize_metadata_string(summary.get("target_model")),
            "supportedModels": list(summary.get("supported_models") or []),
            "source": self.sources.source_metadata_for_summary(summary),
            "contentHash": content_hash,
            "updatedAt": normalize_metadata_string(summary.get("mtime")),
        }
        log_cube_library_diagnostic(
            "sugarcubes_catalog_entry",
            cube_id=cube_id,
            version=entry["version"],
            content_hash=entry["contentHash"],
            source_kind=(
                entry["source"].get("kind", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
            source_path=(
                entry["source"].get("path", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
        )
        if icon:
            entry["icon"] = dict(icon)
        if payload:
            requirements = iter_custom_node_slugs(payload)
            if requirements:
                entry["requiredCustomNodes"] = list(requirements)
        return entry

    def summary_payload_with_hash(
        self,
        summary: Mapping[str, Any],
    ) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]:
        """Return payload and hash from internal summary facts or a fallback read."""

        if "_content_hash" in summary:
            payload = summary.get("_payload")
            return (
                dict(payload) if isinstance(payload, Mapping) else None,
                normalize_metadata_string(summary.get("error")) or None,
                normalize_metadata_string(summary.get("_content_hash")),
            )
        cube_id = normalize_metadata_string(summary.get("cube_id"))
        cube_path = self.sources.resolve_cube_by_id(cube_id)
        return read_cube_payload_with_hash(cube_path)

    def pack_record(self, repo_entry: Mapping[str, Any]) -> dict[str, Any]:
        """Return an API-safe Cube Pack record from SugarCubes repo state."""

        owner = normalize_metadata_string(repo_entry.get("owner"))
        repo = normalize_metadata_string(repo_entry.get("repo"))
        repo_ref = (
            normalize_metadata_string(repo_entry.get("repo_ref")) or f"{owner}/{repo}"
        )
        checkout_path_raw = normalize_metadata_string(
            repo_entry.get("local_checkout_path")
        )
        checkout_path = (
            Path(checkout_path_raw) if checkout_path_raw else Path("__missing__")
        )
        return {
            "repoRef": repo_ref,
            "owner": owner,
            "repo": repo,
            "branch": normalize_metadata_string(repo_entry.get("branch")) or "main",
            "enabled": bool(repo_entry.get("enabled")),
            "defaultBaseRepo": bool(repo_entry.get("default_base_repo")),
            "autoUpdate": bool(repo_entry.get("auto_update")),
            "localHeadSha": normalize_metadata_string(repo_entry.get("local_head_sha")),
            "remoteHeadSha": normalize_metadata_string(
                repo_entry.get("remote_head_sha")
            ),
            "updateAvailable": bool(repo_entry.get("update_available")),
            "lastSyncAt": normalize_metadata_string(repo_entry.get("last_sync_at")),
            "lastSyncStatus": normalize_metadata_string(
                repo_entry.get("last_sync_status")
            )
            or "never",
            "lastSyncError": normalize_metadata_string(
                repo_entry.get("last_sync_error")
            ),
            "lastCheckedAt": normalize_metadata_string(
                repo_entry.get("last_checked_at")
            ),
            "lastCheckStatus": normalize_metadata_string(
                repo_entry.get("last_check_status")
            )
            or "never",
            "lastCheckError": normalize_metadata_string(
                repo_entry.get("last_check_error")
            ),
            "cubeCount": self._count_cube_files(checkout_path),
        }

    def pack_counts(self) -> dict[str, int]:
        """Return count metadata for tracked Cube Packs."""

        repos = self.tracked_repo_service.list_repos()["repos"]
        return {
            "count": len(repos),
            "enabledCount": sum(1 for repo in repos if repo.get("enabled")),
        }

    def revision_pack_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]:
        """Return normalized pack facts used to compute catalog revisions."""

        facts: list[dict[str, Any]] = []
        for repo in self.tracked_repo_service.list_repos()["repos"]:
            if not include_disabled and not repo.get("enabled"):
                continue
            facts.append(
                {
                    "repo_ref": repo.get("repo_ref"),
                    "branch": repo.get("branch"),
                    "enabled": bool(repo.get("enabled")),
                    "local_head_sha": repo.get("local_head_sha"),
                    "remote_head_sha": repo.get("remote_head_sha"),
                    "update_available": bool(repo.get("update_available")),
                }
            )
        return sorted(facts, key=lambda fact: str(fact.get("repo_ref", "")).casefold())

    def _count_cube_files(self, root: Path) -> int:
        """Return the number of loadable cube files under one checkout path."""

        if not root.exists() or not root.is_dir():
            return 0
        return len([path for path in list_cube_files(root) if ".git" not in path.parts])
