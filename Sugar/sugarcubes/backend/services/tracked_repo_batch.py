#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Coordinate batch sync and update checks for tracked repositories."""

from __future__ import annotations

from collections.abc import Callable
from datetime import datetime, timezone
from typing import Any

from ..responses import BackendError
from .tracked_repo_catalog import TrackedRepoCatalog
from .tracked_repo_manifest import TrackedRepoManifest
from .tracked_repo_models import serialize_tracked_repo

RepoOperation = Callable[..., dict[str, Any]]


class TrackedRepoBatch:
    """Apply single-repository lifecycle operations across the catalog."""

    def __init__(
        self,
        *,
        manifest: TrackedRepoManifest,
        catalog: TrackedRepoCatalog,
        sync_repo: RepoOperation,
        check_repo: RepoOperation,
    ) -> None:
        """Initialize batch coordination with single-repository operations."""

        self._manifest = manifest
        self._catalog = catalog
        self._sync_repo = sync_repo
        self._check_repo = check_repo

    def sync_all(self) -> dict[str, Any]:
        """Sync every enabled tracked repository and collect results."""

        results: list[dict[str, Any]] = []
        for repo in self._manifest.load():
            if not repo.enabled:
                continue
            try:
                results.append(
                    self._sync_repo(owner=repo.owner, repo=repo.repo)["repo"]
                )
            except BackendError as error:
                results.append(
                    {
                        "owner": repo.owner,
                        "repo": repo.repo,
                        "branch": repo.branch,
                        "enabled": repo.enabled,
                        "default_base_repo": repo.default_base_repo,
                        "auto_update": repo.auto_update,
                        "local_checkout_path": repo.local_checkout_path,
                        "last_sync_status": "error",
                        "last_sync_error": error.message,
                        "last_sync_at": _utc_now(),
                        "last_checked_at": repo.last_checked_at,
                        "last_check_status": repo.last_check_status,
                        "last_check_error": repo.last_check_error,
                        "remote_head_sha": repo.remote_head_sha,
                        "local_head_sha": repo.local_head_sha,
                        "update_available": repo.update_available,
                    }
                )
        return {"repos": results, "count": len(results)}

    def check_all(self, *, apply_auto_updates: bool) -> dict[str, Any]:
        """Refresh all update states and optionally apply automatic updates."""

        results: list[dict[str, Any]] = []
        for tracked in self._manifest.load():
            if not tracked.enabled:
                results.append(serialize_tracked_repo(tracked))
                continue
            try:
                checked = self._check_repo(owner=tracked.owner, repo=tracked.repo)[
                    "repo"
                ]
                if (
                    apply_auto_updates
                    and checked.get("auto_update") is True
                    and checked.get("update_available") is True
                ):
                    checked = self._sync_repo(owner=tracked.owner, repo=tracked.repo)[
                        "repo"
                    ]
                results.append(checked)
            except BackendError as error:
                current = self._catalog.get_repo(tracked.owner, tracked.repo)
                results.append(
                    {
                        **serialize_tracked_repo(current),
                        "last_checked_at": _utc_now(),
                        "last_check_status": "error",
                        "last_check_error": error.message,
                    }
                )
        return {"repos": results, "count": len(results)}


def _utc_now() -> str:
    """Return the current UTC timestamp for repository status metadata."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")
