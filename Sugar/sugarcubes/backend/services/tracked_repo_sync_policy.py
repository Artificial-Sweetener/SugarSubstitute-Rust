#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Protect authored tracked-repository state during synchronization."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from ..responses import BackendError
from .tracked_repo_git import TrackedRepoGit
from .tracked_repo_models import TrackedRepo


class TrackedRepoSyncPolicy:
    """Reject sync operations that would discard local authored state."""

    def __init__(
        self,
        *,
        git: TrackedRepoGit,
        protected_owner_provider: Callable[[], str] | None,
    ) -> None:
        """Initialize sync safeguards with explicit Git and owner boundaries."""

        self._git = git
        self._protected_owner_provider = protected_owner_provider

    def assert_clean_checkout(self, checkout: Path) -> None:
        """Reject destructive sync when a tracked checkout is dirty."""

        if self._git.is_dirty(checkout):
            raise BackendError(
                "Tracked repo has local changes; commit or discard them before syncing",
                status=409,
                details={"checkout": str(checkout)},
            )

    def assert_preserves_local_commits(
        self, tracked: TrackedRepo, checkout: Path
    ) -> None:
        """Reject protected sync when local commits are ahead of the remote."""

        if not self._must_preserve_local_commits(tracked):
            return
        if self._git.local_head_is_ancestor_of_remote(tracked, checkout):
            return
        raise BackendError(
            "Cannot sync protected tracked repo because local commits are ahead of the remote; push or merge them before syncing",
            status=409,
            details={
                "repo": tracked.repo_ref,
                "checkout": str(checkout),
                "branch": tracked.branch,
                "local_head_sha": self._git.local_head_sha(checkout),
                "remote_head_sha": self._git.ref_sha(
                    checkout, f"origin/{tracked.branch}"
                ),
            },
        )

    def _must_preserve_local_commits(self, tracked: TrackedRepo) -> bool:
        """Return whether sync must preserve local commits for this repo."""

        if tracked.default_base_repo or self._protected_owner_provider is None:
            return tracked.default_base_repo
        protected_owner = self._protected_owner_provider().strip()
        return (
            bool(protected_owner) and protected_owner.lower() == tracked.owner.lower()
        )
