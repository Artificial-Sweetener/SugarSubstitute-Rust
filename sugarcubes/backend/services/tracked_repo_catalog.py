#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Own tracked-repository catalog mutations and preflight policy."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Optional

from ..responses import BackendError
from .tracked_repo_git import TrackedRepoGit
from .tracked_repo_manifest import TrackedRepoManifest
from .tracked_repo_models import (
    DEFAULT_BRANCH,
    TrackedRepo,
    is_default_base_repo,
    normalize_branch_name,
    serialize_tracked_repo,
)
from .tracked_repo_preflight_service import (
    TrackedRepoPreflight,
    TrackedRepoPreflightResult,
)


class TrackedRepoCatalog:
    """Manage tracked repository definitions independently of Git lifecycle."""

    def __init__(
        self,
        *,
        manifest: TrackedRepoManifest,
        preflight: TrackedRepoPreflight,
        git: TrackedRepoGit,
    ) -> None:
        """Initialize catalog persistence and remote preflight boundaries."""

        self._manifest = manifest
        self._preflight = preflight
        self._git = git

    def list_repos(self) -> dict[str, Any]:
        """Return the tracked repository listing payload."""

        repos = [serialize_tracked_repo(repo) for repo in self._manifest.load()]
        return {
            "repos": repos,
            "count": len(repos),
            "workspace_root": str(self._manifest.workspace_root),
        }

    def get_repo(self, owner: str, repo: str) -> TrackedRepo:
        """Return one tracked repository or raise not found."""

        normalized_owner, normalized_repo = self._manifest.normalize_repo_ref(
            owner, repo
        )
        for tracked in self._manifest.load():
            if tracked.owner == normalized_owner and tracked.repo == normalized_repo:
                return tracked
        raise BackendError(
            f"Tracked repo '{normalized_owner}/{normalized_repo}' not found", status=404
        )

    def add_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
        enabled: bool,
        auto_update: bool,
    ) -> dict[str, Any]:
        """Create one tracked repository entry after cube preflight."""

        normalized_owner, normalized_repo = self._manifest.normalize_repo_ref(
            owner, repo
        )
        normalized_branch = normalize_branch_name(branch)
        repos = self._manifest.load()
        if any(
            entry.owner == normalized_owner and entry.repo == normalized_repo
            for entry in repos
        ):
            raise BackendError(
                f"Tracked repo '{normalized_owner}/{normalized_repo}' already exists",
                status=409,
            )
        preflight = self.require_repo_contains_cubes(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
        )
        next_entry = TrackedRepo(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
            enabled=enabled,
            default_base_repo=is_default_base_repo(normalized_owner, normalized_repo),
            auto_update=bool(auto_update),
            local_checkout_path=str(
                self._manifest.checkout_path(normalized_owner, normalized_repo)
            ),
        )
        repos.append(next_entry)
        self._manifest.write(repos)
        return {
            "repo": serialize_tracked_repo(next_entry),
            "preflight": preflight.to_payload(),
        }

    def preflight_repo(self, *, owner: str, repo: str, branch: str) -> dict[str, Any]:
        """Return cube preflight results without writing catalog state."""

        normalized_owner, normalized_repo = self._manifest.normalize_repo_ref(
            owner, repo
        )
        result = self.require_repo_contains_cubes(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalize_branch_name(branch),
        )
        return {"preflight": result.to_payload()}

    def ensure_authoring_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Ensure an authoring repository has an initialized local checkout."""

        normalized_owner, normalized_repo = self._manifest.normalize_repo_ref(
            owner, repo
        )
        repos = self._manifest.load()
        existing = next(
            (
                entry
                for entry in repos
                if entry.owner == normalized_owner and entry.repo == normalized_repo
            ),
            None,
        )
        if existing is not None:
            if not existing.enabled:
                raise BackendError(
                    f"Tracked repo '{existing.repo_ref}' is disabled; enable it before saving into it",
                    status=409,
                )
            repo_entry = existing
        else:
            repo_entry = TrackedRepo(
                owner=normalized_owner,
                repo=normalized_repo,
                branch=normalize_branch_name(branch),
                enabled=True,
                default_base_repo=is_default_base_repo(
                    normalized_owner, normalized_repo
                ),
                auto_update=False,
                local_checkout_path=str(
                    self._manifest.checkout_path(normalized_owner, normalized_repo)
                ),
            )
            repos.append(repo_entry)
            self._manifest.write(repos)
        checkout_path = Path(
            repo_entry.local_checkout_path
            or self._manifest.checkout_path(normalized_owner, normalized_repo)
        )
        self._git.ensure_repo(checkout_path, branch=DEFAULT_BRANCH)
        return {"repo": serialize_tracked_repo(repo_entry)}

    def require_repo_contains_cubes(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Require one remote repository to contain at least one cube."""

        return self._preflight.require_cubes(owner=owner, repo=repo, branch=branch)

    def update_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: Optional[str],
        enabled: Optional[bool],
        auto_update: Optional[bool],
    ) -> dict[str, Any]:
        """Update one tracked repository entry."""

        current = self.get_repo(owner, repo)
        replacement = TrackedRepo(
            owner=current.owner,
            repo=current.repo,
            branch=(
                normalize_branch_name(branch) if branch is not None else current.branch
            ),
            enabled=current.enabled if enabled is None else bool(enabled),
            default_base_repo=current.default_base_repo,
            auto_update=(
                current.auto_update if auto_update is None else bool(auto_update)
            ),
            local_checkout_path=current.local_checkout_path,
            last_sync_at=current.last_sync_at,
            last_sync_status=current.last_sync_status,
            last_sync_error=current.last_sync_error,
            last_checked_at=current.last_checked_at,
            last_check_status=current.last_check_status,
            last_check_error=current.last_check_error,
            remote_head_sha=current.remote_head_sha,
            local_head_sha=current.local_head_sha,
            update_available=current.update_available,
        )
        replacement = self._manifest.normalize(replacement)
        self._manifest.write(
            (
                replacement
                if entry.owner == current.owner and entry.repo == current.repo
                else entry
            )
            for entry in self._manifest.load()
        )
        return {"repo": serialize_tracked_repo(replacement)}

    def remove_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove one tracked repository entry from the manifest."""

        current = self.get_repo(owner, repo)
        if current.default_base_repo:
            raise BackendError(
                "The Base-Cubes repository is always tracked as the default base repo",
                status=409,
            )
        self._manifest.write(
            entry
            for entry in self._manifest.load()
            if not (entry.owner == current.owner and entry.repo == current.repo)
        )
        return {"removed": {"owner": current.owner, "repo": current.repo}}
