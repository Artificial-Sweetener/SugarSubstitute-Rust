#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Adapt legacy command-recording test doubles to repository semantics."""

from __future__ import annotations

from collections.abc import Callable, Sequence
import configparser
from pathlib import Path

from sugarcubes.backend.services.repository_service import RepositoryCommit
from sugarcubes.backend.services.repository_service import RepositoryOperationError
from sugarcubes.backend.services.repository_service import (
    RepositoryReferenceNotFoundError,
)

CommandRunner = Callable[..., object]


class CommandRepository:
    """Preserve command-observation tests without retaining runtime commands."""

    def __init__(self, runner: CommandRunner | None) -> None:
        """Store an optional command double used only by tests."""

        self._runner = runner

    def initialize(self, repository_path: Path, *, branch: str) -> None:
        """Record repository initialization."""

        self._run(["init", "-b", branch], repository_path)

    def clone(
        self,
        repository_url: str,
        target_path: Path,
        *,
        branch: str | None = None,
        depth: int = 0,
    ) -> None:
        """Record a clone using the historical observable arguments."""

        command = ["clone"]
        if depth:
            command.extend([f"--depth={depth}", "--filter=blob:none", "--no-checkout"])
        if branch:
            command.extend(["--branch", branch])
        command.extend([repository_url, str(target_path)])
        try:
            self._run(command, target_path.parent)
        except RuntimeError as exc:
            if "Remote branch" in str(exc) and "not found" in str(exc):
                raise RepositoryReferenceNotFoundError(str(exc)) from exc
            raise RepositoryOperationError(str(exc)) from exc

    def fetch(
        self,
        repository_path: Path,
        *,
        remote_name: str = "origin",
        branch: str | None = None,
        tags: bool = True,
    ) -> None:
        """Record a fetch operation."""

        _ = tags
        command = (
            ["fetch", remote_name, branch] if branch else ["fetch", "--all", "--tags"]
        )
        try:
            self._run(command, repository_path)
        except RuntimeError as exc:
            raise RepositoryOperationError(str(exc)) from exc

    def hard_reset(self, repository_path: Path, revision: str) -> None:
        """Record a hard reset."""

        self._run(["reset", "--hard", revision], repository_path)

    def checkout_revision(self, repository_path: Path, revision: str) -> None:
        """Record a detached or exact checkout."""

        self._run(["checkout", "--detach", revision], repository_path)

    def head_commit_id(self, repository_path: Path) -> str:
        """Return recorded HEAD output."""

        head_path = repository_path / ".git" / "HEAD"
        try:
            head = head_path.read_text(encoding="utf-8").strip()
        except OSError:
            head = ""
        if head and not head.startswith("ref:"):
            return head
        if head.startswith("ref:"):
            ref_path = repository_path / ".git" / head.removeprefix("ref:").strip()
            try:
                return ref_path.read_text(encoding="utf-8").strip()
            except OSError:
                pass
        return self._stdout(["rev-parse", "HEAD"], repository_path)

    def revision_commit_id(self, repository_path: Path, revision: str) -> str:
        """Return recorded revision output or the requested fake revision."""

        output = self._stdout(
            ["cat-file", "-e", f"{revision}^{{commit}}"], repository_path
        )
        return output or revision

    def remote_branch_commit_id(self, repository_url: str, branch: str) -> str:
        """Return recorded remote-head output."""

        output = self._stdout(
            ["ls-remote", "--heads", repository_url, branch],
            Path.cwd(),
        )
        return output.split()[0] if output else ""

    def remote_url(self, repository_path: Path, remote_name: str = "origin") -> str:
        """Return a recorded remote URL."""

        parser = configparser.ConfigParser()
        try:
            parser.read(repository_path / ".git" / "config", encoding="utf-8")
            configured = parser.get(
                f'remote "{remote_name}"',
                "url",
                fallback="",
            ).strip()
        except (OSError, configparser.Error):
            configured = ""
        if configured:
            return configured
        return self._stdout(
            ["config", "--get", f"remote.{remote_name}.url"],
            repository_path,
        )

    def is_dirty(self, repository_path: Path) -> bool:
        """Return recorded porcelain status."""

        return bool(self._stdout(["status", "--porcelain"], repository_path))

    def changed_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return paths parsed from recorded porcelain status."""

        return tuple(
            path
            for line in self._output(
                ["status", "--porcelain"], repository_path
            ).splitlines()
            if (path := _status_path(line))
        )

    def has_path_changes(self, repository_path: Path, relative_path: str) -> bool:
        """Return recorded path-scoped status."""

        return bool(
            self._stdout(
                ["status", "--porcelain", "--", relative_path],
                repository_path,
            )
        )

    def staged_paths(self, repository_path: Path) -> tuple[str, ...]:
        """Return recorded staged paths."""

        return tuple(
            line.strip()
            for line in self._stdout(
                ["diff", "--cached", "--name-only"], repository_path
            ).splitlines()
            if line.strip()
        )

    def stage_paths(self, repository_path: Path, relative_paths: Sequence[str]) -> None:
        """Record staging selected paths."""

        self._run(["add", "--", *relative_paths], repository_path)

    def unstage_paths(
        self, repository_path: Path, relative_paths: Sequence[str]
    ) -> None:
        """Record restoring selected index entries."""

        self._run(["reset", "HEAD", "--", *relative_paths], repository_path)

    def commit_staged(
        self,
        repository_path: Path,
        *,
        message: str,
        author_name: str,
        author_email: str,
    ) -> str:
        """Record a commit and return its resulting HEAD."""

        self._run(
            [
                "-c",
                f"user.name={author_name}",
                "-c",
                f"user.email={author_email}",
                "commit",
                "-m",
                message,
            ],
            repository_path,
        )
        return self.head_commit_id(repository_path)

    def is_ancestor(
        self, repository_path: Path, ancestor: str, descendant: str
    ) -> bool:
        """Return whether the command double accepts an ancestry check."""

        try:
            result = self._run(
                ["merge-base", "--is-ancestor", ancestor, descendant],
                repository_path,
            )
        except RuntimeError:
            return False
        return int(getattr(result, "returncode", 0) or 0) == 0

    def history_for_path(
        self, repository_path: Path, relative_path: str
    ) -> tuple[RepositoryCommit, ...]:
        """Parse historical log output from the command double."""

        output = self._stdout(
            ["log", "--format=%H%x1f%cI%x1f%s", "--", relative_path],
            repository_path,
        )
        commits: list[RepositoryCommit] = []
        for line in output.splitlines():
            parts = line.split("\x1f")
            if len(parts) == 3:
                commits.append(RepositoryCommit(parts[0], parts[1], parts[2]))
        return tuple(commits)

    def read_file_at_revision(
        self, repository_path: Path, revision: str, relative_path: str
    ) -> str:
        """Return historical file output from the command double."""

        return self._output(["show", f"{revision}:{relative_path}"], repository_path)

    def tree_paths(
        self, repository_path: Path, revision: str = "HEAD"
    ) -> tuple[str, ...]:
        """Return recorded tree paths."""

        return tuple(
            line.strip()
            for line in self._stdout(
                ["ls-tree", "-r", "--name-only", revision], repository_path
            ).splitlines()
            if line.strip()
        )

    def _stdout(self, command: list[str], cwd: Path) -> str:
        """Return normalized stdout from one recorded operation."""

        return self._output(command, cwd).strip()

    def _output(self, command: list[str], cwd: Path) -> str:
        """Return exact stdout from one recorded operation."""

        result = self._run(command, cwd)
        return str(getattr(result, "stdout", "") or "")

    def _run(self, command: list[str], cwd: Path) -> object:
        """Invoke the command double or return an inert result."""

        if self._runner is None:
            return object()
        return self._runner(command, cwd=cwd)


def _status_path(line: str) -> str:
    """Extract a normalized path from one porcelain status line."""

    value = line[3:].strip() if len(line) > 3 else ""
    if " -> " in value:
        value = value.split(" -> ", 1)[1]
    return value.strip('"').replace("\\", "/")


__all__ = ["CommandRepository"]
