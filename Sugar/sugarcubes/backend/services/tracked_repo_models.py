#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Define tracked-repository state and immutable normalization policy."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

DEFAULT_BRANCH = "main"
DEFAULT_BASE_OWNER = "Artificial-Sweetener"
DEFAULT_BASE_REPO = "Base-Cubes"


@dataclass(frozen=True)
class TrackedRepo:
    """Represent one locally tracked GitHub cube repo."""

    owner: str
    repo: str
    branch: str
    enabled: bool = True
    default_base_repo: bool = False
    auto_update: bool = False
    local_checkout_path: str = ""
    last_sync_at: str = ""
    last_sync_status: str = "never"
    last_sync_error: str = ""
    last_checked_at: str = ""
    last_check_status: str = "never"
    last_check_error: str = ""
    remote_head_sha: str = ""
    local_head_sha: str = ""
    update_available: bool = False

    @property
    def repo_ref(self) -> str:
        """Return `owner/repo` for UI and lookup flows."""

        return f"{self.owner}/{self.repo}"

    @property
    def remote_url(self) -> str:
        """Return the canonical GitHub remote URL."""

        return f"https://github.com/{self.owner}/{self.repo}.git"


@dataclass(frozen=True)
class CubeCommitResult:
    """Describe one Git commit created for saved cube files."""

    commit_sha: str
    commit_short_sha: str
    commit_message: str


def normalize_branch_name(value: str) -> str:
    """Return the only supported tracked branch name."""

    _ = value
    return DEFAULT_BRANCH


def is_default_base_repo(owner: str, repo: str) -> bool:
    """Return whether a repo is the repository-standard default base repo."""

    return owner == DEFAULT_BASE_OWNER and repo == DEFAULT_BASE_REPO


def normalize_tracked_repo(repo: TrackedRepo) -> TrackedRepo:
    """Normalize immutable tracked-repository invariants."""

    is_default = is_default_base_repo(repo.owner, repo.repo)
    return TrackedRepo(
        owner=repo.owner,
        repo=repo.repo,
        branch=repo.branch,
        enabled=True if is_default else repo.enabled,
        default_base_repo=is_default,
        auto_update=repo.auto_update,
        local_checkout_path=repo.local_checkout_path,
        last_sync_at=repo.last_sync_at,
        last_sync_status=repo.last_sync_status,
        last_sync_error=repo.last_sync_error,
        last_checked_at=repo.last_checked_at,
        last_check_status=repo.last_check_status,
        last_check_error=repo.last_check_error,
        remote_head_sha=repo.remote_head_sha,
        local_head_sha=repo.local_head_sha,
        update_available=repo.update_available,
    )


def serialize_tracked_repo(repo: TrackedRepo) -> dict[str, Any]:
    """Serialize one tracked repo for JSON responses and storage."""

    return {
        "owner": repo.owner,
        "repo": repo.repo,
        "branch": repo.branch,
        "enabled": repo.enabled,
        "default_base_repo": is_default_base_repo(repo.owner, repo.repo),
        "auto_update": repo.auto_update,
        "local_checkout_path": repo.local_checkout_path,
        "last_sync_at": repo.last_sync_at,
        "last_sync_status": repo.last_sync_status,
        "last_sync_error": repo.last_sync_error,
        "last_checked_at": repo.last_checked_at,
        "last_check_status": repo.last_check_status,
        "last_check_error": repo.last_check_error,
        "remote_head_sha": repo.remote_head_sha,
        "local_head_sha": repo.local_head_sha,
        "update_available": repo.update_available,
        "repo_ref": repo.repo_ref,
        "remote_url": repo.remote_url,
    }
