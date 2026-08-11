#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Adapt Git commands used by tracked-repository workflows."""

from __future__ import annotations

import subprocess
from collections.abc import Sequence
from pathlib import Path

from ..responses import BackendError
from .tracked_repo_models import CubeCommitResult, TrackedRepo
from .tracked_repo_preflight_service import GitRunner

_GIT_TIMEOUT_SECONDS = 30


class TrackedRepoGit:
    """Execute tracked-repository Git inspection and mutation commands."""

    def __init__(self, *, runner: GitRunner, workspace_root: Path) -> None:
        """Initialize Git execution with an explicit runner and workspace."""

        self.runner = runner
        self._workspace_root = workspace_root

    def ensure_repo(self, repo_root: Path, *, branch: str) -> Path:
        """Ensure a local repository exists and is Git-initialized."""

        if (repo_root / ".git").exists():
            return repo_root
        repo_root.mkdir(parents=True, exist_ok=True)
        self.runner(["init", "-b", branch], cwd=repo_root)
        return repo_root

    def list_staged_paths(self, repo_root: Path) -> list[str]:
        """Return staged paths for one managed repository."""

        result = self.runner(["diff", "--cached", "--name-only"], cwd=repo_root)
        return [
            line.strip() for line in (result.stdout or "").splitlines() if line.strip()
        ]

    def has_file_changes(self, repo_root: Path, repo_relative_path: str) -> bool:
        """Return whether one repository-relative path has visible changes."""

        try:
            result = self.runner(
                ["status", "--porcelain", "--", repo_relative_path],
                cwd=repo_root,
            )
        except RuntimeError as exc:
            raise BackendError(
                "Failed to inspect saved cube git status",
                status=500,
                details={
                    "repo_root": str(repo_root),
                    "repo_relative_path": repo_relative_path,
                    "reason": str(exc),
                },
            ) from exc
        return bool((result.stdout or "").strip())

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
            self.runner(["add", "--", *normalized_paths], cwd=repo_root)
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
            self.runner(
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
            head = self.runner(["rev-parse", "HEAD"], cwd=repo_root)
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

    def clone_checkout(self, tracked: TrackedRepo, checkout: Path) -> None:
        """Clone a tracked repo, tolerating an empty remote without branches."""

        try:
            self.runner(
                [
                    "clone",
                    "--branch",
                    tracked.branch,
                    tracked.remote_url,
                    str(checkout),
                ],
                cwd=self._workspace_root,
            )
        except RuntimeError as exc:
            reason = str(exc)
            missing_branch = "Remote branch" in reason and "not found" in reason
            empty_remote = "does not appear to have any commits yet" in reason
            if not missing_branch and not empty_remote:
                raise
            self.runner(
                ["clone", tracked.remote_url, str(checkout)],
                cwd=self._workspace_root,
            )

    def local_head_sha(self, checkout: Path) -> str:
        """Return the local HEAD SHA for one managed checkout."""

        result = self.runner(["rev-parse", "HEAD"], cwd=checkout)
        return (result.stdout or "").strip()

    def ref_sha(self, checkout: Path, ref: str) -> str:
        """Return a checkout ref SHA, or empty when unavailable."""

        try:
            result = self.runner(["rev-parse", ref], cwd=checkout)
        except RuntimeError:
            return ""
        return (result.stdout or "").strip()

    def remote_head_sha(self, tracked: TrackedRepo) -> str:
        """Return the remote branch HEAD without updating the checkout."""

        result = self.runner(
            ["ls-remote", "--heads", tracked.remote_url, tracked.branch],
            cwd=self._workspace_root,
        )
        first_line = next(
            (
                line.strip()
                for line in (result.stdout or "").splitlines()
                if line.strip()
            ),
            "",
        )
        return first_line.split()[0].strip() if first_line else ""

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


def run_git(args: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run one Git subprocess with explicit arguments and a timeout."""

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
        reason = (exc.stderr or "").strip() or (exc.stdout or "").strip() or str(exc)
        raise RuntimeError(reason) from exc
    except subprocess.TimeoutExpired as exc:
        raise RuntimeError("git command timed out") from exc
