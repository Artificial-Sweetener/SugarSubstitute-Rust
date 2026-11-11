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
"""Coordinate Cube Pack lifecycle operations and response projection."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from .cube_library_catalog_projection import CubeLibraryCatalogProjection
from .tracked_repo_service import TrackedRepoService


class CubeLibraryPackService:
    """Own tracked Cube Pack lifecycle orchestration."""

    def __init__(
        self,
        *,
        tracked_repo_service: TrackedRepoService,
        projection: CubeLibraryCatalogProjection,
        invalidate_catalog: Callable[..., None],
        catalog_revision: Callable[..., str],
    ) -> None:
        """Initialize pack operations with catalog state callbacks."""

        self.tracked_repo_service = tracked_repo_service
        self.projection = projection
        self.invalidate_catalog_state = invalidate_catalog
        self.catalog_revision = catalog_revision

    def list_library_packs(self) -> dict[str, Any]:
        """Return tracked Cube Pack records without exposing checkout paths."""

        packs = [
            self.projection.pack_record(repo_entry)
            for repo_entry in self.tracked_repo_service.list_repos()["repos"]
        ]
        packs.sort(key=lambda pack: str(pack.get("repoRef", "")).casefold())
        return {
            "schemaVersion": 1,
            "packs": packs,
            "catalogRevision": self.catalog_revision(),
        }

    def preflight_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Return preflight information for a candidate Cube Pack."""

        payload = self.tracked_repo_service.preflight_repo(
            owner=owner,
            repo=repo,
            branch=branch,
        )
        return {"schemaVersion": 1, **payload}

    def add_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
        enabled: bool,
        auto_update: bool,
        sync_immediately: bool,
    ) -> dict[str, Any]:
        """Track a Cube Pack and optionally perform the first synchronous sync."""

        payload = self.tracked_repo_service.add_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            default_base_repo=False,
            auto_update=auto_update,
        )
        if sync_immediately:
            payload = {
                **payload,
                "repo": self.tracked_repo_service.sync_repo(
                    owner=owner,
                    repo=repo,
                )["repo"],
            }
        self.invalidate_catalog_state(reason="pack_added")
        return {
            "schemaVersion": 1,
            "pack": self.projection.pack_record(payload["repo"]),
            "preflight": payload.get("preflight", {}),
            "catalogRevision": self.catalog_revision(),
        }

    def update_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str | None,
        enabled: bool | None,
        auto_update: bool | None,
    ) -> dict[str, Any]:
        """Update a tracked Cube Pack and return refreshed library state."""

        payload = self.tracked_repo_service.update_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )
        self.invalidate_catalog_state(reason="pack_updated")
        return {
            "schemaVersion": 1,
            "pack": self.projection.pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def remove_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove a tracked Cube Pack through SugarCubes policy enforcement."""

        payload = self.tracked_repo_service.remove_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_removed")
        return {
            "schemaVersion": 1,
            **payload,
            "catalogRevision": self.catalog_revision(),
        }

    def sync_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Synchronously sync one tracked Cube Pack."""

        payload = self.tracked_repo_service.sync_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_synced")
        return {
            "schemaVersion": 1,
            "pack": self.projection.pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def sync_all_library_packs(self) -> dict[str, Any]:
        """Synchronously sync all enabled Cube Packs and return per-pack results."""

        payload = self.tracked_repo_service.sync_all_repos()
        self.invalidate_catalog_state(reason="all_packs_synced")
        return {
            "schemaVersion": 1,
            "packs": [self.projection.pack_record(repo) for repo in payload["repos"]],
            "catalogRevision": self.catalog_revision(),
        }
