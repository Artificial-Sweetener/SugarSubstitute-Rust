#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Implement SugarCubes repository operations with bundled libgit2."""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pygit2

from .repository_network import RepositoryNetworkClient
from .repository_service import RepositoryCommit, RepositoryOperationError
from .pygit2_tree import tree_entry_id, walk_tree_paths

_CHECKOUT_STRATEGY = pygit2.GIT_CHECKOUT_SAFE | pygit2.GIT_CHECKOUT_RECREATE_MISSING


class Pygit2RepositoryService:
    """Own repository access without requiring a system Git executable."""

    def __init__(self, *, network: RepositoryNetworkClient | None = None) -> None:
        """Initialize local and bounded remote repository operations."""

        self._network = network or RepositoryNetworkClient()

    def initialize(self, repository_path: Path, *, branch: str) -> None:
        """Initialize an empty repository at the requested branch."""

        repository_path.mkdir(parents=True, exist_ok=True)
        try:
            repository = pygit2.init_repository(repository_path, initial_head=branch)
            repository.free()
        except (OSError, ValueError, pygit2.GitError) as exc:
            raise RepositoryOperationError(
                f"Could not initialize repository at {repository_path}: {exc}"
            ) from exc

    def clone(
        self,
        repository_url: str,
        target_path: Path,
        *,
        branch: str | None = None,
        depth: int = 0,
    ) -> None:
        """Clone a repository through libgit2 and remove partial output on failure."""

        target_path.parent.mkdir(parents=True, exist_ok=True)
        self._network.clone(
            repository_url,
            target_path,
            branch=branch,
            depth=depth,
        )

    def fetch(
        self,
        repository_path: Path,
        *,
        remote_name: str = "origin",
        branch: str | None = None,
        tags: bool = True,
    ) -> None:
        """Fetch an optional branch and tags through libgit2."""

        self._network.fetch(
            repository_path,
            remote_name=remote_name,
            branch=branch,
            tags=tags,
        )

    def hard_reset(self, repository_path: Path, revision: str) -> None:
        """Reset the index and worktree to an exact local revision."""

        with _open_repository(repository_path) as repository:
            commit = _resolve_commit(repository, revision)
            try:
                repository.reset(commit.id, pygit2.enums.ResetMode.HARD)
            except (ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not reset {repository_path} to {revision}: {exc}"
                ) from exc

    def checkout_revision(self, repository_path: Path, revision: str) -> None:
        """Check out one exact revision with a detached head."""

        with _open_repository(repository_path) as repository:
            commit = _resolve_commit(repository, revision)
            try:
                repository.checkout_tree(  # type: ignore[no-untyped-call]
                    commit,
                    strategy=_CHECKOUT_STRATEGY,
                )
                repository.set_head(commit.id)
            except (ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not checkout {revision} in {repository_path}: {exc}"
                ) from exc

    def head_commit_id(self, repository_path: Path) -> str:
        """Return the current commit identifier or an empty string."""

        with _open_repository(repository_path) as repository:
            if repository.head_is_unborn:
                return ""
            try:
                return str(repository.head.target)
            except (KeyError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not resolve HEAD in {repository_path}: {exc}"
                ) from exc

    def revision_commit_id(self, repository_path: Path, revision: str) -> str:
        """Return one resolved commit identifier or an empty string."""

        with _open_repository(repository_path) as repository:
            try:
                return str(repository.revparse_single(revision).peel(pygit2.Commit).id)
            except KeyError:
                return ""
            except (ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not resolve {revision} in {repository_path}: {exc}"
                ) from exc

    def remote_branch_commit_id(self, repository_url: str, branch: str) -> str:
        """Return a remote branch head without changing a managed checkout."""

        return self._network.remote_branch_commit_id(repository_url, branch)

    def remote_url(self, repository_path: Path, remote_name: str = "origin") -> str:
        """Return the configured remote URL or an empty string."""

        with _open_repository(repository_path) as repository:
            try:
                return repository.remotes[remote_name].url or ""
            except (KeyError, ValueError):
                return ""

    def is_dirty(self, repository_path: Path) -> bool:
        """Return whether tracked or untracked worktree state differs."""

        with _open_repository(repository_path) as repository:
            return any(
                flags != pygit2.GIT_STATUS_CURRENT
                for flags in repository.status().values()
            )

    def changed_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return normalized paths with staged or worktree changes."""

        with _open_repository(repository_path) as repository:
            return tuple(
                sorted(
                    path.replace("\\", "/")
                    for path, flags in repository.status().items()
                    if flags != pygit2.GIT_STATUS_CURRENT
                )
            )

    def has_path_changes(self, repository_path: Path, relative_path: str) -> bool:
        """Return whether one path has staged or worktree changes."""

        with _open_repository(repository_path) as repository:
            try:
                return (
                    repository.status_file(relative_path) != pygit2.GIT_STATUS_CURRENT
                )
            except KeyError:
                return False

    def staged_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return normalized paths staged relative to HEAD."""

        with _open_repository(repository_path) as repository:
            try:
                if repository.head_is_unborn:
                    return tuple(sorted(entry.path for entry in repository.index))
                diff = repository.index.diff_to_tree(
                    repository.head.peel(pygit2.Commit).tree
                )
                return tuple(
                    sorted(
                        delta.new_file.path or delta.old_file.path
                        for delta in diff.deltas
                    )
                )
            except (KeyError, ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not inspect staged paths in {repository_path}: {exc}"
                ) from exc

    def stage_paths(self, repository_path: Path, relative_paths: Sequence[str]) -> None:
        """Stage additions, modifications, and deletions for selected paths."""

        with _open_repository(repository_path) as repository:
            try:
                index = repository.index
                for relative_path in relative_paths:
                    if (repository_path / relative_path).exists():
                        index.add(relative_path)
                    else:
                        try:
                            index.remove(relative_path)
                        except KeyError:
                            pass
                index.write()
            except (OSError, ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not stage paths in {repository_path}: {exc}"
                ) from exc

    def unstage_paths(
        self, repository_path: Path, relative_paths: Sequence[str]
    ) -> None:
        """Restore selected index paths to HEAD without changing the worktree."""

        with _open_repository(repository_path) as repository:
            try:
                index = repository.index
                if repository.head_is_unborn:
                    for relative_path in relative_paths:
                        try:
                            index.remove(relative_path)
                        except KeyError:
                            pass
                else:
                    tree = repository.head.peel(pygit2.Commit).tree
                    for relative_path in relative_paths:
                        try:
                            entry = tree[relative_path]
                        except KeyError:
                            try:
                                index.remove(relative_path)
                            except KeyError:
                                pass
                        else:
                            index.add(
                                pygit2.IndexEntry(
                                    relative_path,
                                    entry.id,
                                    entry.filemode,
                                )
                            )
                index.write()
            except (OSError, ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not unstage paths in {repository_path}: {exc}"
                ) from exc

    def commit_staged(
        self,
        repository_path: Path,
        *,
        message: str,
        author_name: str,
        author_email: str,
    ) -> str:
        """Create a commit from the current index and return its identifier."""

        with _open_repository(repository_path) as repository:
            try:
                tree_id = repository.index.write_tree()
                parents = [] if repository.head_is_unborn else [repository.head.target]
                signature = pygit2.Signature(author_name, author_email)
                commit_id = repository.create_commit(
                    "HEAD",
                    signature,
                    signature,
                    message,
                    tree_id,
                    parents,
                )
                return str(commit_id)
            except (KeyError, ValueError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not commit staged paths in {repository_path}: {exc}"
                ) from exc

    def is_ancestor(
        self, repository_path: Path, ancestor: str, descendant: str
    ) -> bool:
        """Return whether one revision is an ancestor of another."""

        with _open_repository(repository_path) as repository:
            ancestor_commit = _resolve_commit(repository, ancestor)
            descendant_commit = _resolve_commit(repository, descendant)
            return (
                ancestor_commit.id == descendant_commit.id
                or repository.descendant_of(
                    descendant_commit.id,
                    ancestor_commit.id,
                )
            )

    def history_for_path(
        self, repository_path: Path, relative_path: str
    ) -> tuple[RepositoryCommit, ...]:
        """Return newest-first commits that changed one path."""

        with _open_repository(repository_path) as repository:
            if repository.head_is_unborn:
                return ()
            commits: list[RepositoryCommit] = []
            for commit in repository.walk(
                repository.head.target,
                pygit2.enums.SortMode.TOPOLOGICAL | pygit2.enums.SortMode.TIME,
            ):
                current_id = tree_entry_id(commit.tree, relative_path)
                parent_id = (
                    tree_entry_id(commit.parents[0].tree, relative_path)
                    if commit.parents
                    else None
                )
                if current_id == parent_id:
                    continue
                offset = timezone(timedelta(minutes=commit.commit_time_offset))
                committed_at = datetime.fromtimestamp(
                    commit.commit_time,
                    tz=offset,
                ).isoformat()
                commits.append(
                    RepositoryCommit(
                        commit_id=str(commit.id),
                        committed_at=committed_at,
                        message=commit.message.strip(),
                    )
                )
            return tuple(commits)

    def read_file_at_revision(
        self, repository_path: Path, revision: str, relative_path: str
    ) -> str:
        """Return UTF-8 text for one repository file at a revision."""

        with _open_repository(repository_path) as repository:
            commit = _resolve_commit(repository, revision)
            try:
                entry = commit.tree[relative_path]
                blob = repository[entry.id].peel(pygit2.Blob)
                return bytes(blob.data).decode("utf-8")
            except (KeyError, UnicodeDecodeError, pygit2.GitError) as exc:
                raise RepositoryOperationError(
                    f"Could not read {relative_path} at {revision}: {exc}"
                ) from exc

    def tree_paths(
        self, repository_path: Path, revision: str = "HEAD"
    ) -> tuple[str, ...]:
        """Return every file path in a revision tree."""

        with _open_repository(repository_path) as repository:
            commit = _resolve_commit(repository, revision)
            return tuple(walk_tree_paths(repository, commit.tree))


@contextmanager
def _open_repository(repository_path: Path) -> Iterator[pygit2.Repository]:
    """Open a repository and release native handles deterministically."""

    repository: pygit2.Repository | None = None
    try:
        git_directory = pygit2.discover_repository(repository_path)
        if git_directory is None:
            raise RepositoryOperationError(
                f"No repository exists at {repository_path}."
            )
        repository = pygit2.Repository(git_directory)
        yield repository
    except RepositoryOperationError:
        raise
    except (OSError, ValueError, pygit2.GitError) as exc:
        raise RepositoryOperationError(
            f"Could not open repository {repository_path}: {exc}"
        ) from exc
    finally:
        if repository is not None:
            repository.free()


def _resolve_commit(repository: pygit2.Repository, revision: str) -> pygit2.Commit:
    """Resolve one revision to a commit or raise an actionable error."""

    try:
        return repository.revparse_single(revision).peel(pygit2.Commit)
    except (KeyError, ValueError, pygit2.GitError) as exc:
        raise RepositoryOperationError(
            f"Unknown repository revision: {revision}"
        ) from exc


__all__ = ["Pygit2RepositoryService"]
