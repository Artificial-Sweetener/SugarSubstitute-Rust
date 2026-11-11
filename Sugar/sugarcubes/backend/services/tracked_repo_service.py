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
"""Tracked GitHub repo persistence, sync, and git commit services."""

from __future__ import annotations

from datetime import datetime, timezone
import logging
from pathlib import Path
from typing import Any, Callable, Optional, Sequence

from ..responses import BackendError
from .pygit2_repository import Pygit2RepositoryService
from .repository_service import RepositoryCommit, RepositoryService
from .tracked_repo_batch import TrackedRepoBatch
from .tracked_repo_catalog import TrackedRepoCatalog
from .tracked_repo_git import (
    TrackedRepoGit,
    normalize_repo_relative_path,
)
from .tracked_repo_manifest import TrackedRepoManifest
from .tracked_repo_models import (
    DEFAULT_BRANCH,
    CubeCommitResult,
    TrackedRepo,
    serialize_tracked_repo,
)
from .tracked_repo_preflight_service import (
    TrackedRepoPreflight,
    TrackedRepoPreflightResult,
    TrackedRepoPreflightService,
    list_local_cube_candidate_paths,
)
from .tracked_repo_sync_policy import TrackedRepoSyncPolicy

_logger = logging.getLogger(__name__)


class TrackedRepoService:
    """Own tracked GitHub repo manifest state and git sync behavior."""

    def __init__(
        self,
        extension_root: Path,
        *,
        repositories: RepositoryService | None = None,
        preflight_service: TrackedRepoPreflight | None = None,
        protected_owner_provider: Optional[Callable[[], str]] = None,
    ) -> None:
        """Initialize the tracked repo service."""

        self.extension_root = extension_root.resolve()
        self.repositories = repositories or Pygit2RepositoryService()
        self.protected_owner_provider = protected_owner_provider
        self._manifest = TrackedRepoManifest(self.extension_root)
        self._git = TrackedRepoGit(repositories=self.repositories)
        self._sync_policy = TrackedRepoSyncPolicy(
            git=self._git,
            protected_owner_provider=protected_owner_provider,
        )
        self.preflight_service = preflight_service or TrackedRepoPreflightService(
            workspace_root=self.workspace_root(),
            repositories=self.repositories,
        )
        self._catalog = TrackedRepoCatalog(
            manifest=self._manifest,
            preflight=self.preflight_service,
            git=self._git,
        )
        self._batch = TrackedRepoBatch(
            manifest=self._manifest,
            catalog=self._catalog,
            sync_repo=self.sync_repo,
            check_repo=self.check_repo,
        )

    def data_root(self) -> Path:
        """Return the extension-owned data root."""

        return self._manifest.data_root

    def manifest_path(self) -> Path:
        """Return the tracked repo manifest path."""

        return self._manifest.path

    def workspace_root(self) -> Path:
        """Return the managed source root for tracked GitHub repo checkouts."""

        return self._manifest.workspace_root

    def local_repo_root(self) -> Path:
        """Return the managed local source repo root."""

        return self._manifest.local_repo_root

    def ensure_local_repo(self) -> Path:
        """Ensure the managed local source repo exists and is git-initialized."""

        return self._git.ensure_repo(self.local_repo_root(), branch=DEFAULT_BRANCH)

    def checkout_path(self, owner: str, repo: str) -> Path:
        """Return the managed local checkout path for a tracked repo."""

        return self._manifest.checkout_path(owner, repo)

    def list_staged_paths(self, *, repo_root: Path) -> list[str]:
        """Return staged paths for one managed repo."""

        return self._git.list_staged_paths(repo_root)

    def has_file_changes(self, *, repo_root: Path, repo_relative_path: str) -> bool:
        """Return whether one repo-relative path has git-visible changes."""

        return self._git.has_file_changes(
            repo_root,
            normalize_repo_relative_path(repo_relative_path),
        )

    def changed_paths(self, *, repo_root: Path) -> tuple[str, ...]:
        """Return normalized paths with staged or worktree changes."""

        return self.repositories.changed_paths(repo_root)

    def commit_file(
        self,
        *,
        repo_root: Path,
        repo_relative_path: str,
        commit_message: str,
    ) -> CubeCommitResult:
        """Stage and commit one file without including unrelated staged changes."""

        return self.commit_paths(
            repo_root=repo_root,
            repo_relative_paths=[repo_relative_path],
            commit_message=commit_message,
        )

    def commit_paths(
        self,
        *,
        repo_root: Path,
        repo_relative_paths: Sequence[str],
        commit_message: str,
    ) -> CubeCommitResult:
        """Commit one cohesive path set without including unrelated staged changes."""

        return self._git.commit_paths(
            repo_root=repo_root,
            repo_relative_paths=repo_relative_paths,
            commit_message=commit_message,
        )

    def unstage_paths(
        self, *, repo_root: Path, repo_relative_paths: Sequence[str]
    ) -> None:
        """Restore selected index entries without changing saved files."""

        self._git.unstage_paths(repo_root, repo_relative_paths)

    def history_for_path(
        self, *, repo_root: Path, repo_relative_path: str
    ) -> tuple[RepositoryCommit, ...]:
        """Return newest-first commits that changed one repository path."""

        return self.repositories.history_for_path(
            repo_root,
            normalize_repo_relative_path(repo_relative_path),
        )

    def read_file_at_revision(
        self, *, repo_root: Path, revision: str, repo_relative_path: str
    ) -> str:
        """Return one historical repository file as UTF-8 text."""

        return self.repositories.read_file_at_revision(
            repo_root,
            revision,
            normalize_repo_relative_path(repo_relative_path),
        )

    def head_commit_id(self, *, repo_root: Path) -> str:
        """Return the current repository commit identifier."""

        return self.repositories.head_commit_id(repo_root)

    def list_repos(self) -> dict[str, Any]:
        """Return the tracked repo listing payload."""

        return self._catalog.list_repos()

    def get_repo(self, owner: str, repo: str) -> TrackedRepo:
        """Return one tracked repo or raise a not-found backend error."""

        return self._catalog.get_repo(owner, repo)

    def add_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
        enabled: bool,
        default_base_repo: bool,
        auto_update: bool = False,
    ) -> dict[str, Any]:
        """Create one tracked repo entry in the manifest."""

        _ = default_base_repo
        return self._catalog.add_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )

    def preflight_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Return cube preflight results without writing tracked repo state."""

        return self._catalog.preflight_repo(owner=owner, repo=repo, branch=branch)

    def require_repo_contains_cubes(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Require one remote tracked repo to contain at least one cube."""

        return self._catalog.require_repo_contains_cubes(
            owner=owner, repo=repo, branch=branch
        )

    def ensure_authoring_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str = DEFAULT_BRANCH,
    ) -> dict[str, Any]:
        """Ensure one tracked authoring repo has an initialized local checkout."""

        return self._catalog.ensure_authoring_repo(
            owner=owner,
            repo=repo,
            branch=branch,
        )

    def update_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: Optional[str] = None,
        enabled: Optional[bool] = None,
        default_base_repo: Optional[bool] = None,
        auto_update: Optional[bool] = None,
    ) -> dict[str, Any]:
        """Update one tracked repo entry."""

        _ = default_base_repo
        return self._catalog.update_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )

    def remove_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove one tracked repo entry from the manifest."""

        return self._catalog.remove_repo(owner=owner, repo=repo)

    def sync_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Clone or fast-forward one tracked repo checkout."""

        tracked = self.get_repo(owner, repo)
        if not tracked.enabled:
            raise BackendError(
                f"Tracked repo '{tracked.repo_ref}' is disabled", status=409
            )
        checkout = Path(tracked.local_checkout_path or self.checkout_path(owner, repo))
        checkout.parent.mkdir(parents=True, exist_ok=True)
        try:
            if not checkout.exists():
                self._git.clone_checkout(tracked, checkout)
            else:
                self._sync_policy.assert_clean_checkout(checkout)
                self._git.fetch_branch(tracked, checkout)
                self._sync_policy.assert_preserves_local_commits(tracked, checkout)
                self._git.hard_reset_to_remote(tracked, checkout)
            cube_paths = list_local_cube_candidate_paths(checkout)
            if not cube_paths:
                self._manifest.replace(
                    tracked,
                    last_sync_status="error",
                    last_sync_error=(
                        f"Repository '{tracked.repo_ref}' does not contain any .cube files "
                        f"on branch '{tracked.branch}'."
                    ),
                    last_sync_at=_utc_now(),
                    local_checkout_path=str(checkout),
                )
                raise BackendError(
                    f"Repository '{tracked.repo_ref}' does not contain any .cube files on branch '{tracked.branch}'.",
                    status=422,
                    details={
                        "repo": tracked.repo_ref,
                        "branch": tracked.branch,
                        "reason": "no_cubes",
                    },
                )
            local_head_sha = self._git.local_head_sha(checkout)
            refreshed = self._manifest.replace(
                tracked,
                last_sync_status="ok",
                last_sync_error="",
                last_sync_at=_utc_now(),
                local_checkout_path=str(checkout),
                local_head_sha=local_head_sha,
                remote_head_sha=local_head_sha,
                update_available=False,
            )
        except BackendError:
            raise
        except (OSError, RuntimeError, ValueError) as exc:
            refreshed = self._manifest.replace(
                tracked,
                last_sync_status="error",
                last_sync_error=str(exc),
                last_sync_at=_utc_now(),
                local_checkout_path=str(checkout),
            )
            self._logger_sync_failure(tracked.repo_ref, exc)
            raise BackendError(
                f"Failed to sync tracked repo '{tracked.repo_ref}'",
                status=500,
                details={"repo": tracked.repo_ref, "reason": str(exc)},
            ) from exc
        return {"repo": serialize_tracked_repo(refreshed)}

    def sync_all_repos(self) -> dict[str, Any]:
        """Sync every enabled tracked repo and return individual results."""

        return self._batch.sync_all()

    def check_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Refresh update availability for one tracked pack without updating it."""

        tracked = self.get_repo(owner, repo)
        checkout = Path(tracked.local_checkout_path or self.checkout_path(owner, repo))
        try:
            local_head_sha = (
                self._git.local_head_sha(checkout) if checkout.exists() else ""
            )
            remote_head_sha = self._git.remote_head_sha(tracked)
            update_available = (
                bool(remote_head_sha) and remote_head_sha != local_head_sha
            )
            refreshed = self._manifest.replace(
                tracked,
                local_checkout_path=str(checkout),
                last_checked_at=_utc_now(),
                last_check_status="ok",
                last_check_error="",
                local_head_sha=local_head_sha,
                remote_head_sha=remote_head_sha,
                update_available=update_available,
            )
        except BackendError:
            raise
        except (OSError, RuntimeError, ValueError) as exc:
            refreshed = self._manifest.replace(
                tracked,
                local_checkout_path=str(checkout),
                last_checked_at=_utc_now(),
                last_check_status="error",
                last_check_error=str(exc),
            )
            _logger.exception(
                "SugarCubes: failed to check tracked repo %s",
                tracked.repo_ref,
                exc_info=exc,
            )
            raise BackendError(
                f"Failed to check tracked repo '{tracked.repo_ref}' for updates",
                status=500,
                details={"repo": tracked.repo_ref, "reason": str(exc)},
            ) from exc
        return {"repo": serialize_tracked_repo(refreshed)}

    def check_all_repos(self, *, apply_auto_updates: bool = False) -> dict[str, Any]:
        """Refresh update state for every tracked pack, optionally auto-updating."""

        return self._batch.check_all(apply_auto_updates=apply_auto_updates)

    def _logger_sync_failure(self, repo_ref: str, exc: Exception) -> None:
        """Log one actionable tracked repo sync failure."""

        _logger.exception(
            "SugarCubes: failed to sync tracked repo %s", repo_ref, exc_info=exc
        )


def _utc_now() -> str:
    """Return the current UTC timestamp for manifest sync metadata."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")
