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

from dataclasses import asdict, dataclass
from datetime import datetime, timezone
import json
import logging
from pathlib import Path
import subprocess
from typing import Any, Callable, Iterable, Optional, Sequence

from ...cube_model.cube_identity import (
    CubeIdentityError,
    RESERVED_SOURCE_NAMES,
    validate_github_repo_ref,
)
from ..responses import BackendError
from .tracked_repo_preflight_service import (
    GitRunner,
    TrackedRepoPreflight,
    TrackedRepoPreflightResult,
    TrackedRepoPreflightService,
    list_local_cube_candidate_paths,
)

_logger = logging.getLogger(__name__)

_DEFAULT_BRANCH = "main"
_MANIFEST_DIRNAME = ".sugarcubes"
_MANIFEST_NAME = "tracked_repos.json"
_GIT_TIMEOUT_SECONDS = 30
_DEFAULT_BASE_OWNER = "Artificial-Sweetener"
_DEFAULT_BASE_REPO = "Base-Cubes"


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
    """Describe one git commit created for a saved cube file."""

    commit_sha: str
    commit_short_sha: str
    commit_message: str


class TrackedRepoService:
    """Own tracked GitHub repo manifest state and git sync behavior."""

    def __init__(
        self,
        extension_root: Path,
        *,
        git_runner: GitRunner | None = None,
        preflight_service: TrackedRepoPreflight | None = None,
        protected_owner_provider: Optional[Callable[[], str]] = None,
    ) -> None:
        """Initialize the tracked repo service."""

        self.extension_root = extension_root.resolve()
        self.git_runner = git_runner or _run_git
        self.protected_owner_provider = protected_owner_provider
        self.preflight_service = preflight_service or TrackedRepoPreflightService(
            workspace_root=self.workspace_root(),
            git_runner=self.git_runner,
        )

    def data_root(self) -> Path:
        """Return the extension-owned data root."""

        return self.extension_root / _MANIFEST_DIRNAME

    def manifest_path(self) -> Path:
        """Return the tracked repo manifest path."""

        return self.data_root() / _MANIFEST_NAME

    def workspace_root(self) -> Path:
        """Return the managed source root for tracked GitHub repo checkouts."""

        return self.data_root()

    def local_repo_root(self) -> Path:
        """Return the managed local source repo root."""

        return self.data_root() / "local"

    def ensure_local_repo(self) -> Path:
        """Ensure the managed local source repo exists and is git-initialized."""

        local_root = self.local_repo_root()
        git_dir = local_root / ".git"
        if git_dir.exists():
            return local_root
        local_root.mkdir(parents=True, exist_ok=True)
        self.git_runner(["init", "-b", _DEFAULT_BRANCH], cwd=local_root)
        return local_root

    def checkout_path(self, owner: str, repo: str) -> Path:
        """Return the managed local checkout path for a tracked repo."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        return self.workspace_root() / normalized_owner / normalized_repo

    def list_staged_paths(self, *, repo_root: Path) -> list[str]:
        """Return staged paths for one managed repo."""

        result = self.git_runner(["diff", "--cached", "--name-only"], cwd=repo_root)
        return [
            line.strip() for line in (result.stdout or "").splitlines() if line.strip()
        ]

    def has_file_changes(self, *, repo_root: Path, repo_relative_path: str) -> bool:
        """Return whether one repo-relative path has git-visible changes."""

        normalized_path = self._normalize_repo_relative_path(repo_relative_path)
        try:
            result = self.git_runner(
                ["status", "--porcelain", "--", normalized_path],
                cwd=repo_root,
            )
        except RuntimeError as exc:
            raise BackendError(
                "Failed to inspect saved cube git status",
                status=500,
                details={
                    "repo_root": str(repo_root),
                    "repo_relative_path": normalized_path,
                    "reason": str(exc),
                },
            ) from exc
        return bool((result.stdout or "").strip())

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

        normalized_paths = tuple(
            dict.fromkeys(
                self._normalize_repo_relative_path(path) for path in repo_relative_paths
            )
        )
        if not normalized_paths:
            raise BackendError(
                "At least one repo-relative path is required", status=400
            )
        allowed_paths = set(normalized_paths)
        try:
            staged_before = self.list_staged_paths(repo_root=repo_root)
            unrelated_staged = [
                path
                for path in staged_before
                if self._normalize_repo_relative_path(path) not in allowed_paths
            ]
            if unrelated_staged:
                raise BackendError(
                    "Repo has unrelated staged changes; commit them separately before saving this cube",
                    status=409,
                    details={
                        "repo_root": str(repo_root),
                        "staged_paths": unrelated_staged,
                    },
                )

            self.git_runner(["add", "--", *normalized_paths], cwd=repo_root)
            staged_after = self.list_staged_paths(repo_root=repo_root)
            unrelated_after = [
                path
                for path in staged_after
                if self._normalize_repo_relative_path(path) not in allowed_paths
            ]
            if unrelated_after:
                raise BackendError(
                    "Repo has unrelated staged changes; commit them separately before saving this cube",
                    status=409,
                    details={
                        "repo_root": str(repo_root),
                        "staged_paths": unrelated_after,
                    },
                )
            staged_normalized = {
                self._normalize_repo_relative_path(path) for path in staged_after
            }
            if not staged_normalized.intersection(allowed_paths):
                raise BackendError(
                    "Saved cube mutation did not produce a staged git diff",
                    status=409,
                    details={
                        "repo_root": str(repo_root),
                        "repo_relative_paths": list(normalized_paths),
                    },
                )
            self.git_runner(
                [
                    "-c",
                    "user.name=SugarCubes",
                    "-c",
                    "user.email=sugarcubes@example.invalid",
                    "commit",
                    "-m",
                    commit_message,
                ],
                cwd=repo_root,
            )
            head = self.git_runner(["rev-parse", "HEAD"], cwd=repo_root)
        except BackendError:
            raise
        except RuntimeError as exc:
            raise BackendError(
                "Failed to commit saved cube revision",
                status=500,
                details={
                    "repo_root": str(repo_root),
                    "repo_relative_paths": list(normalized_paths),
                    "reason": str(exc),
                },
            ) from exc
        commit_sha = (head.stdout or "").strip()
        return CubeCommitResult(
            commit_sha=commit_sha,
            commit_short_sha=commit_sha[:7],
            commit_message=commit_message,
        )

    def list_repos(self) -> dict[str, Any]:
        """Return the tracked repo listing payload."""

        repos = [self._serialize(repo) for repo in self._load_manifest()]
        return {
            "repos": repos,
            "count": len(repos),
            "workspace_root": str(self.workspace_root()),
        }

    def get_repo(self, owner: str, repo: str) -> TrackedRepo:
        """Return one tracked repo or raise a not-found backend error."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        for tracked in self._load_manifest():
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
        default_base_repo: bool,
        auto_update: bool = False,
    ) -> dict[str, Any]:
        """Create one tracked repo entry in the manifest."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        normalized_branch = normalize_branch_name(branch)
        repos = self._load_manifest()
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
        checkout_path = self.checkout_path(normalized_owner, normalized_repo)
        next_entry = TrackedRepo(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
            enabled=enabled,
            default_base_repo=_is_default_base_repo(normalized_owner, normalized_repo),
            auto_update=bool(auto_update),
            local_checkout_path=str(checkout_path),
        )
        repos.append(next_entry)
        self._write_manifest(repos)
        return {
            "repo": self._serialize(next_entry),
            "preflight": preflight.to_payload(),
        }

    def preflight_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Return cube preflight results without writing tracked repo state."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        normalized_branch = normalize_branch_name(branch)
        result = self.require_repo_contains_cubes(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
        )
        return {"preflight": result.to_payload()}

    def require_repo_contains_cubes(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Require one remote tracked repo to contain at least one cube."""

        return self.preflight_service.require_cubes(
            owner=owner,
            repo=repo,
            branch=branch,
        )

    def ensure_authoring_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str = _DEFAULT_BRANCH,
    ) -> dict[str, Any]:
        """Ensure one tracked authoring repo has an initialized local checkout."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        normalized_branch = normalize_branch_name(branch)
        repos = self._load_manifest()
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
                branch=normalized_branch,
                enabled=True,
                default_base_repo=_is_default_base_repo(
                    normalized_owner, normalized_repo
                ),
                auto_update=False,
                local_checkout_path=str(
                    self.checkout_path(normalized_owner, normalized_repo)
                ),
            )
            repos.append(repo_entry)
            self._write_manifest(repos)

        checkout_path = Path(
            repo_entry.local_checkout_path
            or self.checkout_path(normalized_owner, normalized_repo)
        )
        checkout_path.mkdir(parents=True, exist_ok=True)
        if not (checkout_path / ".git").exists():
            self.git_runner(["init", "-b", _DEFAULT_BRANCH], cwd=checkout_path)
        return {"repo": self._serialize(repo_entry)}

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

        current = self.get_repo(owner, repo)
        repos = self._load_manifest()
        next_branch = (
            normalize_branch_name(branch) if branch is not None else current.branch
        )
        next_enabled = current.enabled if enabled is None else bool(enabled)
        next_default = (
            current.default_base_repo
            if default_base_repo is None
            else _is_default_base_repo(current.owner, current.repo)
        )
        next_auto_update = (
            current.auto_update if auto_update is None else bool(auto_update)
        )
        replacement = TrackedRepo(
            owner=current.owner,
            repo=current.repo,
            branch=next_branch,
            enabled=next_enabled,
            default_base_repo=next_default,
            auto_update=next_auto_update,
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
        replacement = self._normalize_repo(replacement)
        rewritten = [
            (
                replacement
                if entry.owner == current.owner and entry.repo == current.repo
                else entry
            )
            for entry in repos
        ]
        self._write_manifest(rewritten)
        return {"repo": self._serialize(replacement)}

    def remove_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove one tracked repo entry from the manifest."""

        current = self.get_repo(owner, repo)
        if current.default_base_repo:
            raise BackendError(
                "The Base-Cubes repository is always tracked as the default base repo",
                status=409,
            )
        repos = [
            entry
            for entry in self._load_manifest()
            if not (entry.owner == current.owner and entry.repo == current.repo)
        ]
        self._write_manifest(repos)
        return {"removed": {"owner": current.owner, "repo": current.repo}}

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
                self._clone_checkout(tracked, checkout)
            else:
                self._assert_clean_checkout(checkout)
                self.git_runner(["fetch", "origin", tracked.branch], cwd=checkout)
                self._assert_sync_will_not_discard_author_commits(tracked, checkout)
                self.git_runner(
                    ["reset", "--hard", f"origin/{tracked.branch}"],
                    cwd=checkout,
                )
            cube_paths = list_local_cube_candidate_paths(checkout)
            if not cube_paths:
                self._replace_repo_state(
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
            local_head_sha = self._resolve_local_head_sha(checkout)
            refreshed = self._replace_repo_state(
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
            refreshed = self._replace_repo_state(
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
        return {"repo": self._serialize(refreshed)}

    def sync_all_repos(self) -> dict[str, Any]:
        """Sync every enabled tracked repo and return individual results."""

        results: list[dict[str, Any]] = []
        for repo in self._load_manifest():
            if not repo.enabled:
                continue
            try:
                results.append(self.sync_repo(owner=repo.owner, repo=repo.repo)["repo"])
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

    def check_repo(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Refresh update availability for one tracked pack without updating it."""

        tracked = self.get_repo(owner, repo)
        checkout = Path(tracked.local_checkout_path or self.checkout_path(owner, repo))
        try:
            local_head_sha = (
                self._resolve_local_head_sha(checkout) if checkout.exists() else ""
            )
            remote_head_sha = self._resolve_remote_head_sha(tracked)
            update_available = (
                bool(remote_head_sha) and remote_head_sha != local_head_sha
            )
            refreshed = self._replace_repo_state(
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
            refreshed = self._replace_repo_state(
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
        return {"repo": self._serialize(refreshed)}

    def check_all_repos(self, *, apply_auto_updates: bool = False) -> dict[str, Any]:
        """Refresh update state for every tracked pack, optionally auto-updating."""

        results: list[dict[str, Any]] = []
        for tracked in self._load_manifest():
            if not tracked.enabled:
                results.append(self._serialize(tracked))
                continue
            try:
                checked = self.check_repo(owner=tracked.owner, repo=tracked.repo)[
                    "repo"
                ]
                if (
                    apply_auto_updates
                    and checked.get("auto_update") is True
                    and checked.get("update_available") is True
                ):
                    checked = self.sync_repo(owner=tracked.owner, repo=tracked.repo)[
                        "repo"
                    ]
                results.append(checked)
            except BackendError as error:
                current = self.get_repo(tracked.owner, tracked.repo)
                results.append(
                    {
                        **self._serialize(current),
                        "last_checked_at": _utc_now(),
                        "last_check_status": "error",
                        "last_check_error": error.message,
                    }
                )
        return {"repos": results, "count": len(results)}

    def _assert_clean_checkout(self, checkout: Path) -> None:
        """Reject destructive sync when the local tracked checkout is dirty."""

        result = self.git_runner(["status", "--porcelain"], cwd=checkout)
        if result.stdout.strip():
            raise BackendError(
                "Tracked repo has local changes; commit or discard them before syncing",
                status=409,
                details={"checkout": str(checkout)},
            )

    def _assert_sync_will_not_discard_author_commits(
        self, tracked: TrackedRepo, checkout: Path
    ) -> None:
        """Reject protected sync when local commits are ahead of the remote."""

        if not self._must_preserve_local_commits(tracked):
            return
        remote_ref = f"origin/{tracked.branch}"
        try:
            self.git_runner(
                ["merge-base", "--is-ancestor", "HEAD", remote_ref],
                cwd=checkout,
            )
            return
        except RuntimeError:
            local_head = self._resolve_local_head_sha(checkout)
            remote_head = self._resolve_ref_sha(checkout, remote_ref)
            raise BackendError(
                "Cannot sync protected tracked repo because local commits are ahead "
                "of the remote; push or merge them before syncing",
                status=409,
                details={
                    "repo": tracked.repo_ref,
                    "checkout": str(checkout),
                    "branch": tracked.branch,
                    "local_head_sha": local_head,
                    "remote_head_sha": remote_head,
                },
            )

    def _must_preserve_local_commits(self, tracked: TrackedRepo) -> bool:
        """Return whether sync must not reset local commits for this repo."""

        return tracked.default_base_repo or self._is_protected_author_repo(tracked)

    def _is_protected_author_repo(self, tracked: TrackedRepo) -> bool:
        """Return whether destructive sync must preserve local authored commits."""

        if self.protected_owner_provider is None:
            return False
        protected_owner = self.protected_owner_provider().strip()
        return (
            bool(protected_owner) and protected_owner.lower() == tracked.owner.lower()
        )

    def _load_manifest(self) -> list[TrackedRepo]:
        """Load tracked repo entries from disk."""

        manifest_path = self.manifest_path()
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        if not manifest_path.exists():
            default_repo = self._build_default_base_repo()
            self._write_manifest([default_repo])
            return [default_repo]
        try:
            payload = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise BackendError("Tracked repo manifest is invalid", status=500) from exc
        if not isinstance(payload, dict):
            raise BackendError("Tracked repo manifest is invalid", status=500)
        raw_repos = payload.get("repos")
        if not isinstance(raw_repos, list):
            default_repo = self._build_default_base_repo()
            self._write_manifest([default_repo])
            return [default_repo]
        parsed: list[TrackedRepo] = []
        for entry in raw_repos:
            if not isinstance(entry, dict):
                continue
            owner, repo = validate_github_repo_ref(
                str(entry.get("owner") or ""),
                str(entry.get("repo") or ""),
            )
            parsed.append(
                TrackedRepo(
                    owner=owner,
                    repo=repo,
                    branch=normalize_branch_name(str(entry.get("branch") or "")),
                    enabled=bool(entry.get("enabled", True)),
                    default_base_repo=_is_default_base_repo(owner, repo),
                    auto_update=bool(entry.get("auto_update", False)),
                    local_checkout_path=str(entry.get("local_checkout_path") or ""),
                    last_sync_at=str(entry.get("last_sync_at") or ""),
                    last_sync_status=str(entry.get("last_sync_status") or "never"),
                    last_sync_error=str(entry.get("last_sync_error") or ""),
                    last_checked_at=str(entry.get("last_checked_at") or ""),
                    last_check_status=str(entry.get("last_check_status") or "never"),
                    last_check_error=str(entry.get("last_check_error") or ""),
                    remote_head_sha=str(entry.get("remote_head_sha") or ""),
                    local_head_sha=str(entry.get("local_head_sha") or ""),
                    update_available=bool(entry.get("update_available", False)),
                )
            )
        normalized = self._normalize_manifest_repos(parsed)
        if normalized != parsed:
            self._write_manifest(normalized)
        return normalized

    def _clone_checkout(self, tracked: TrackedRepo, checkout: Path) -> None:
        """Clone one tracked repo, tolerating an empty remote without branches."""

        try:
            self.git_runner(
                [
                    "clone",
                    "--branch",
                    tracked.branch,
                    tracked.remote_url,
                    str(checkout),
                ],
                cwd=self.workspace_root(),
            )
        except RuntimeError as exc:
            reason = str(exc)
            missing_branch = "Remote branch" in reason and "not found" in reason
            empty_remote = "does not appear to have any commits yet" in reason
            if not missing_branch and not empty_remote:
                raise
            self.git_runner(
                ["clone", tracked.remote_url, str(checkout)],
                cwd=self.workspace_root(),
            )

    def _build_default_base_repo(self) -> TrackedRepo:
        """Return the repository-standard default tracked base repo entry."""

        owner, repo = self._normalize_repo_ref(_DEFAULT_BASE_OWNER, _DEFAULT_BASE_REPO)
        return TrackedRepo(
            owner=owner,
            repo=repo,
            branch=_DEFAULT_BRANCH,
            enabled=True,
            default_base_repo=True,
            auto_update=False,
            local_checkout_path=str(self.checkout_path(owner, repo)),
        )

    def _write_manifest(self, repos: Iterable[TrackedRepo]) -> None:
        """Persist tracked repo entries to disk."""

        manifest_path = self.manifest_path()
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        normalized = self._normalize_manifest_repos(list(repos))
        payload = {"repos": [self._serialize(repo) for repo in normalized]}
        manifest_path.write_text(
            json.dumps(payload, indent=2) + "\n",
            encoding="utf-8",
        )

    def _normalize_manifest_repos(
        self, repos: Iterable[TrackedRepo]
    ) -> list[TrackedRepo]:
        """Ensure the canonical Base-Cubes entry exists and owns the base-pack flag."""

        normalized = [self._normalize_repo(repo) for repo in repos]
        if any(_is_default_base_repo(repo.owner, repo.repo) for repo in normalized):
            return normalized
        return [self._build_default_base_repo(), *normalized]

    def _replace_repo_state(self, repo: TrackedRepo, **changes: Any) -> TrackedRepo:
        """Rewrite one tracked repo entry in the persisted manifest."""

        replacement = TrackedRepo(**{**asdict(repo), **changes})
        replacement = self._normalize_repo(replacement)
        rewritten = [
            (
                replacement
                if entry.owner == repo.owner and entry.repo == repo.repo
                else entry
            )
            for entry in self._load_manifest()
        ]
        self._write_manifest(rewritten)
        return replacement

    def _serialize(self, repo: TrackedRepo) -> dict[str, Any]:
        """Serialize one tracked repo for JSON responses and storage."""

        return {
            "owner": repo.owner,
            "repo": repo.repo,
            "branch": repo.branch,
            "enabled": repo.enabled,
            "default_base_repo": _is_default_base_repo(repo.owner, repo.repo),
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

    def _logger_sync_failure(self, repo_ref: str, exc: Exception) -> None:
        """Log one actionable tracked repo sync failure."""

        _logger.exception(
            "SugarCubes: failed to sync tracked repo %s", repo_ref, exc_info=exc
        )

    def _normalize_repo_ref(self, owner: str, repo: str) -> tuple[str, str]:
        """Normalize and validate one GitHub owner/repo reference."""

        owner_candidate = owner.strip() if isinstance(owner, str) else ""
        if owner_candidate.lower() in RESERVED_SOURCE_NAMES:
            raise BackendError(
                f"GitHub owner '{owner_candidate}' is reserved by SugarCubes",
                status=400,
            )
        try:
            normalized_owner, normalized_repo = validate_github_repo_ref(owner, repo)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        return normalized_owner, normalized_repo

    def _normalize_repo_relative_path(self, value: str) -> str:
        """Normalize staged path comparisons to git's forward-slash form."""

        cleaned = value.replace("\\", "/").strip().strip("/")
        if not cleaned:
            raise BackendError("Repo-relative path is required", status=400)
        return cleaned

    def _normalize_repo(self, repo: TrackedRepo) -> TrackedRepo:
        """Normalize tracked-repo invariants that depend on this service root."""

        normalized = _normalize_repo(repo)
        return TrackedRepo(
            owner=normalized.owner,
            repo=normalized.repo,
            branch=normalized.branch,
            enabled=normalized.enabled,
            default_base_repo=normalized.default_base_repo,
            auto_update=normalized.auto_update,
            local_checkout_path=self._normalize_checkout_path(
                owner=normalized.owner,
                repo=normalized.repo,
                persisted_path=normalized.local_checkout_path,
            ),
            last_sync_at=normalized.last_sync_at,
            last_sync_status=normalized.last_sync_status,
            last_sync_error=normalized.last_sync_error,
            last_checked_at=normalized.last_checked_at,
            last_check_status=normalized.last_check_status,
            last_check_error=normalized.last_check_error,
            remote_head_sha=normalized.remote_head_sha,
            local_head_sha=normalized.local_head_sha,
            update_available=normalized.update_available,
        )

    def _normalize_checkout_path(
        self,
        *,
        owner: str,
        repo: str,
        persisted_path: str,
    ) -> str:
        """Return the active managed checkout path for stale or missing manifests."""

        canonical_path = self.checkout_path(owner, repo).resolve()
        if not persisted_path.strip():
            return str(canonical_path)

        persisted = Path(persisted_path).expanduser()
        try:
            resolved_persisted = persisted.resolve()
        except OSError:
            resolved_persisted = persisted.absolute()

        if resolved_persisted == canonical_path:
            return str(canonical_path)
        if not resolved_persisted.exists():
            return str(canonical_path)
        if canonical_path.exists() and _looks_like_managed_checkout_path(
            resolved_persisted,
            owner=owner,
            repo=repo,
        ):
            return str(canonical_path)
        return str(resolved_persisted)

    def _resolve_local_head_sha(self, checkout: Path) -> str:
        """Return the local HEAD SHA for one managed checkout."""

        result = self.git_runner(["rev-parse", "HEAD"], cwd=checkout)
        return (result.stdout or "").strip()

    def _resolve_ref_sha(self, checkout: Path, ref: str) -> str:
        """Return one checkout ref SHA, or an empty string when it is unavailable."""

        try:
            result = self.git_runner(["rev-parse", ref], cwd=checkout)
        except RuntimeError:
            return ""
        return (result.stdout or "").strip()

    def _resolve_remote_head_sha(self, tracked: TrackedRepo) -> str:
        """Return the remote HEAD SHA for the tracked branch without updating the checkout."""

        result = self.git_runner(
            ["ls-remote", "--heads", tracked.remote_url, tracked.branch],
            cwd=self.workspace_root(),
        )
        first_line = next(
            (
                line.strip()
                for line in (result.stdout or "").splitlines()
                if line.strip()
            ),
            "",
        )
        if not first_line:
            return ""
        return first_line.split()[0].strip()


def normalize_branch_name(value: str) -> str:
    """Return the only supported tracked branch name."""

    _ = value
    return _DEFAULT_BRANCH


def _apply_default_repo_flag(
    repos: Iterable[TrackedRepo], default_repo_ref: str
) -> list[TrackedRepo]:
    """Ensure at most one tracked repo is marked as the default base repo."""

    rewritten: list[TrackedRepo] = []
    for repo in repos:
        rewritten.append(
            TrackedRepo(
                owner=repo.owner,
                repo=repo.repo,
                branch=repo.branch,
                enabled=repo.enabled,
                default_base_repo=bool(
                    default_repo_ref and repo.repo_ref == default_repo_ref
                ),
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
        )
    return rewritten


def _is_default_base_repo(owner: str, repo: str) -> bool:
    """Return whether the repo is the repository-standard default base repo."""

    return owner == _DEFAULT_BASE_OWNER and repo == _DEFAULT_BASE_REPO


def _normalize_repo(repo: TrackedRepo) -> TrackedRepo:
    """Normalize immutable tracked-repo invariants."""

    is_default = _is_default_base_repo(repo.owner, repo.repo)
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


def _looks_like_managed_checkout_path(
    path: Path,
    *,
    owner: str,
    repo: str,
) -> bool:
    """Return whether a path follows SugarCubes' managed checkout layout."""

    parts = path.parts
    if len(parts) < 4:
        return False
    return (
        parts[-3].lower() == _MANIFEST_DIRNAME
        and parts[-2] == owner
        and parts[-1] == repo
    )


def _utc_now() -> str:
    """Return the current UTC timestamp for manifest sync metadata."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")


def _run_git(args: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run one git subprocess with explicit argument lists and timeouts."""

    try:
        return subprocess.run(
            ["git", *args],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=_GIT_TIMEOUT_SECONDS,
            check=True,
        )
    except subprocess.CalledProcessError as exc:
        stderr = (exc.stderr or "").strip()
        stdout = (exc.stdout or "").strip()
        reason = stderr or stdout or str(exc)
        raise RuntimeError(reason) from exc
    except subprocess.TimeoutExpired as exc:
        raise RuntimeError("git command timed out") from exc
