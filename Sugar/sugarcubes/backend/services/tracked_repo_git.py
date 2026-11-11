#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Adapt semantic repository operations for tracked-repository workflows."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

from ..responses import BackendError
from .repository_service import (
    RepositoryOperationError,
    RepositoryReferenceNotFoundError,
    RepositoryService,
)
from .tracked_repo_models import CubeCommitResult, TrackedRepo


class TrackedRepoGit:
    """Apply tracked-repository policy through the repository boundary."""

    def __init__(self, *, repositories: RepositoryService) -> None:
        """Initialize repository access through one semantic boundary."""

        self._repositories = repositories

    def ensure_repo(self, repo_root: Path, *, branch: str) -> Path:
        """Ensure a local repository exists and is Git-initialized."""

        if (repo_root / ".git").exists():
            return repo_root
        repo_root.mkdir(parents=True, exist_ok=True)
        self._repositories.initialize(repo_root, branch=branch)
        return repo_root

    def list_staged_paths(self, repo_root: Path) -> list[str]:
        """Return staged paths for one managed repository."""

        return list(self._repositories.staged_paths(repo_root))

    def has_file_changes(self, repo_root: Path, repo_relative_path: str) -> bool:
        """Return whether one repository-relative path has visible changes."""

        try:
            return self._repositories.has_path_changes(
                repo_root,
                repo_relative_path,
            )
        except RepositoryOperationError as exc:
            raise BackendError(
                "Failed to inspect saved cube git status",
                status=500,
                details={
                    "repo_root": str(repo_root),
                    "repo_relative_path": repo_relative_path,
                    "reason": str(exc),
                },
            ) from exc

    def commit_paths(
        self,
        *,
        repo_root: Path,
        repo_relative_paths: Sequence[str],
        commit_message: str,
    ) -> CubeCommitResult:
        """Commit a cohesive path set without unrelated staged changes."""

        normalized_paths = tuple(
            dict.fromkeys(
                normalize_repo_relative_path(path) for path in repo_relative_paths
            )
        )
        if not normalized_paths:
            raise BackendError(
                "At least one repo-relative path is required", status=400
            )
        allowed_paths = set(normalized_paths)
        try:
            self._require_only_allowed_staged(repo_root, allowed_paths)
            self._repositories.stage_paths(repo_root, normalized_paths)
            staged_after = self._require_only_allowed_staged(repo_root, allowed_paths)
            if not set(staged_after).intersection(allowed_paths):
                raise BackendError(
                    "Saved cube mutation did not produce a staged git diff",
                    status=409,
                    details={
                        "repo_root": str(repo_root),
                        "repo_relative_paths": list(normalized_paths),
                    },
                )
            commit_sha = self._repositories.commit_staged(
                repo_root,
                message=commit_message,
                author_name="SugarCubes",
                author_email="sugarcubes@example.invalid",
            )
        except BackendError:
            raise
        except RepositoryOperationError as exc:
            raise BackendError(
                "Failed to commit saved cube revision",
                status=500,
                details={
                    "repo_root": str(repo_root),
                    "repo_relative_paths": list(normalized_paths),
                    "reason": str(exc),
                },
            ) from exc
        return CubeCommitResult(
            commit_sha=commit_sha,
            commit_short_sha=commit_sha[:7],
            commit_message=commit_message,
        )

    def clone_checkout(self, tracked: TrackedRepo, checkout: Path) -> None:
        """Clone a tracked repo, tolerating an empty remote without branches."""

        try:
            self._repositories.clone(
                tracked.remote_url,
                checkout,
                branch=tracked.branch,
            )
        except RepositoryReferenceNotFoundError:
            self._repositories.clone(tracked.remote_url, checkout)

    def local_head_sha(self, checkout: Path) -> str:
        """Return the local HEAD SHA for one managed checkout."""

        return self._repositories.head_commit_id(checkout)

    def ref_sha(self, checkout: Path, ref: str) -> str:
        """Return a checkout ref SHA, or empty when unavailable."""

        return self._repositories.revision_commit_id(checkout, ref)

    def remote_head_sha(self, tracked: TrackedRepo) -> str:
        """Return the remote branch HEAD without updating the checkout."""

        return self._repositories.remote_branch_commit_id(
            tracked.remote_url,
            tracked.branch,
        )

    def fetch_branch(self, tracked: TrackedRepo, checkout: Path) -> None:
        """Fetch one tracked branch and its tags."""

        self._repositories.fetch(checkout, branch=tracked.branch)

    def hard_reset_to_remote(self, tracked: TrackedRepo, checkout: Path) -> None:
        """Reset one clean checkout to its fetched remote branch."""

        self._repositories.hard_reset(checkout, f"origin/{tracked.branch}")

    def is_dirty(self, checkout: Path) -> bool:
        """Return whether one checkout contains local changes."""

        return self._repositories.is_dirty(checkout)

    def local_head_is_ancestor_of_remote(
        self, tracked: TrackedRepo, checkout: Path
    ) -> bool:
        """Return whether sync can retain every local commit."""

        return self._repositories.is_ancestor(
            checkout,
            "HEAD",
            f"origin/{tracked.branch}",
        )

    def unstage_paths(self, repo_root: Path, relative_paths: Sequence[str]) -> None:
        """Restore selected index entries without changing saved files."""

        self._repositories.unstage_paths(repo_root, relative_paths)

    def _require_only_allowed_staged(
        self, repo_root: Path, allowed_paths: set[str]
    ) -> list[str]:
        """Reject staged paths outside an allowed commit set."""

        staged = [
            normalize_repo_relative_path(path)
            for path in self.list_staged_paths(repo_root)
        ]
        unrelated = [path for path in staged if path not in allowed_paths]
        if unrelated:
            raise BackendError(
                "Repo has unrelated staged changes; commit them separately before saving this cube",
                status=409,
                details={"repo_root": str(repo_root), "staged_paths": unrelated},
            )
        return staged


def normalize_repo_relative_path(value: str) -> str:
    """Normalize staged path comparisons to Git's forward-slash form."""

    cleaned = value.replace("\\", "/").strip().strip("/")
    if not cleaned:
        raise BackendError("Repo-relative path is required", status=400)
    return cleaned
