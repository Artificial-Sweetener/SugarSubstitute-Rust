#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Verify real repository behavior without a system Git executable."""

from __future__ import annotations

from pathlib import Path

import pytest
import pygit2

from sugarcubes.backend.services.pygit2_repository import Pygit2RepositoryService
from sugarcubes.backend.services.repository_service import (
    RepositoryReferenceNotFoundError,
)


def test_repository_lifecycle_works_with_empty_path(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Exercise authoring, history, cloning, and checkout through libgit2."""

    monkeypatch.setenv("PATH", "")
    repositories = Pygit2RepositoryService()
    source = tmp_path / "source"
    repositories.initialize(source, branch="main")
    cube_path = source / "example.cube"
    cube_path.write_text('{"version": 1}', encoding="utf-8")

    repositories.stage_paths(source, ("example.cube",))
    first_commit = repositories.commit_staged(
        source,
        message="Add example cube",
        author_name="SugarCubes",
        author_email="sugarcubes@example.invalid",
    )
    cube_path.write_text('{"version": 2}', encoding="utf-8")

    assert repositories.has_path_changes(source, "example.cube") is True
    repositories.stage_paths(source, ("example.cube",))
    second_commit = repositories.commit_staged(
        source,
        message="Update example cube",
        author_name="SugarCubes",
        author_email="sugarcubes@example.invalid",
    )

    history = repositories.history_for_path(source, "example.cube")
    assert [entry.commit_id for entry in history] == [second_commit, first_commit]
    assert (
        repositories.read_file_at_revision(source, first_commit, "example.cube")
        == '{"version": 1}'
    )
    assert repositories.is_ancestor(source, first_commit, second_commit) is True

    checkout = tmp_path / "checkout"
    repositories.clone(str(source), checkout, branch="main")
    assert repositories.head_commit_id(checkout) == second_commit
    assert repositories.remote_branch_commit_id(str(source), "main") == second_commit

    cube_path.write_text('{"version": 3}', encoding="utf-8")
    repositories.stage_paths(source, ("example.cube",))
    third_commit = repositories.commit_staged(
        source,
        message="Update example cube again",
        author_name="SugarCubes",
        author_email="sugarcubes@example.invalid",
    )
    repositories.fetch(checkout, branch="main")
    assert repositories.revision_commit_id(checkout, "origin/main") == third_commit

    repositories.checkout_revision(checkout, first_commit)
    assert repositories.head_commit_id(checkout) == first_commit
    assert (checkout / "example.cube").read_text(encoding="utf-8") == '{"version": 1}'


def test_selected_paths_are_staged_and_unstaged_without_touching_others(
    tmp_path: Path,
) -> None:
    """Preserve unrelated index state while committing a cohesive path set."""

    repositories = Pygit2RepositoryService()
    repository = tmp_path / "repository"
    repositories.initialize(repository, branch="main")
    (repository / "one.cube").write_text("one", encoding="utf-8")
    (repository / "two.cube").write_text("two", encoding="utf-8")

    repositories.stage_paths(repository, ("one.cube", "two.cube"))
    assert repositories.staged_paths(repository) == ("one.cube", "two.cube")
    repositories.unstage_paths(repository, ("two.cube",))
    assert repositories.staged_paths(repository) == ("one.cube",)

    repositories.commit_staged(
        repository,
        message="Add one cube",
        author_name="SugarCubes",
        author_email="sugarcubes@example.invalid",
    )
    assert repositories.has_path_changes(repository, "two.cube") is True


def test_empty_remote_clone_reports_missing_branch_then_clones_repository(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Classify an unborn remote without depending on libgit2 error text upstream."""

    monkeypatch.setenv("PATH", "")
    remote = tmp_path / "remote.git"
    repository = pygit2.init_repository(remote, bare=True, initial_head="main")
    repository.free()
    repositories = Pygit2RepositoryService()
    checkout = tmp_path / "checkout"

    with pytest.raises(RepositoryReferenceNotFoundError):
        repositories.clone(str(remote), checkout, branch="main")

    repositories.clone(str(remote), checkout)

    assert (checkout / ".git").is_dir()
