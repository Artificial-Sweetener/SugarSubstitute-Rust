#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Define semantic repository operations required by SugarCubes workflows."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol


class RepositoryOperationError(RuntimeError):
    """Report a repository operation that could not complete safely."""


class RepositoryReferenceNotFoundError(RepositoryOperationError):
    """Report a requested remote reference that does not exist yet."""


@dataclass(frozen=True, slots=True)
class RepositoryCommit:
    """Describe one commit that changed a selected repository path."""

    commit_id: str
    committed_at: str
    message: str


class RepositoryService(Protocol):
    """Provide Git semantics without exposing executable-shaped commands."""

    def initialize(self, repository_path: Path, *, branch: str) -> None:
        """Initialize an empty repository at the requested branch."""

    def clone(
        self,
        repository_url: str,
        target_path: Path,
        *,
        branch: str | None = None,
        depth: int = 0,
    ) -> None:
        """Clone a repository into a new target directory."""

    def fetch(
        self,
        repository_path: Path,
        *,
        remote_name: str = "origin",
        branch: str | None = None,
        tags: bool = True,
    ) -> None:
        """Fetch an optional branch and tags from one configured remote."""

    def hard_reset(self, repository_path: Path, revision: str) -> None:
        """Reset the index and worktree to an exact local revision."""

    def checkout_revision(self, repository_path: Path, revision: str) -> None:
        """Check out one exact revision with a detached head."""

    def head_commit_id(self, repository_path: Path) -> str:
        """Return the current commit identifier or an empty string."""

    def revision_commit_id(self, repository_path: Path, revision: str) -> str:
        """Return one resolved commit identifier or an empty string."""

    def remote_branch_commit_id(self, repository_url: str, branch: str) -> str:
        """Return a remote branch head without changing a local checkout."""

    def remote_url(self, repository_path: Path, remote_name: str = "origin") -> str:
        """Return the configured remote URL or an empty string."""

    def is_dirty(self, repository_path: Path) -> bool:
        """Return whether tracked or untracked worktree state differs."""

    def changed_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return normalized paths with staged or worktree changes."""

    def has_path_changes(self, repository_path: Path, relative_path: str) -> bool:
        """Return whether one path has staged or worktree changes."""

    def staged_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return normalized paths staged relative to HEAD."""

    def stage_paths(self, repository_path: Path, relative_paths: Sequence[str]) -> None:
        """Stage additions, modifications, and deletions for selected paths."""

    def unstage_paths(
        self, repository_path: Path, relative_paths: Sequence[str]
    ) -> None:
        """Restore selected index paths to HEAD without changing the worktree."""

    def commit_staged(
        self,
        repository_path: Path,
        *,
        message: str,
        author_name: str,
        author_email: str,
    ) -> str:
        """Create a commit from the current index and return its identifier."""

    def is_ancestor(
        self, repository_path: Path, ancestor: str, descendant: str
    ) -> bool:
        """Return whether one revision is an ancestor of another."""

    def history_for_path(
        self, repository_path: Path, relative_path: str
    ) -> tuple[RepositoryCommit, ...]:
        """Return newest-first commits that changed one path."""

    def read_file_at_revision(
        self, repository_path: Path, revision: str, relative_path: str
    ) -> str:
        """Return UTF-8 text for one repository file at a revision."""

    def tree_paths(
        self, repository_path: Path, revision: str = "HEAD"
    ) -> tuple[str, ...]:
        """Return every file path in a revision tree."""


__all__ = [
    "RepositoryCommit",
    "RepositoryOperationError",
    "RepositoryReferenceNotFoundError",
    "RepositoryService",
]
