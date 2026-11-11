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
"""Prove Git-backed governance inputs when the pack is one folder of a larger repository."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tools.architecture.scanner import visible_repository_paths
from tools.architecture.snapshot import repository_snapshot
from tools.testing.selection import changed_paths


def _git(root: Path, *args: str) -> None:
    """Run one fixture Git command with a fixed identity."""

    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.autocrlf=false",
            *args,
        ],
        cwd=root,
        check=True,
        capture_output=True,
        timeout=30,
    )


def _monorepo(tmp_path: Path) -> Path:
    """Commit a pack folder beside a sibling component and return the pack folder."""

    repository = tmp_path / "repository"
    pack = repository / "Pack"
    (pack / "frontend").mkdir(parents=True)
    (repository / "Sibling").mkdir()
    (pack / "frontend/source.ts").write_text(
        "export const value = 1;", encoding="utf-8"
    )
    (pack / ".gitignore").write_text("ignored/\n", encoding="utf-8")
    (repository / "Sibling/host.rs").write_text("fn main() {}", encoding="utf-8")
    _git(repository, "init", "--quiet")
    _git(repository, "add", ".")
    _git(repository, "commit", "--quiet", "-m", "fixture")
    return pack


def test_changes_are_reported_relative_to_the_nested_pack(tmp_path: Path) -> None:
    """Select the pack's own changes by pack-relative path and ignore sibling components."""

    pack = _monorepo(tmp_path)
    (pack / "frontend/source.ts").write_text(
        "export const value = 2;", encoding="utf-8"
    )
    (pack.parent / "Sibling/host.rs").write_text("fn main() { }", encoding="utf-8")
    (pack / "frontend/added.ts").write_text("export {};", encoding="utf-8")
    assert changed_paths(pack, staged=False) == (
        "frontend/added.ts",
        "frontend/source.ts",
    )
    _git(pack.parent, "add", ".")
    assert changed_paths(pack, staged=True) == (
        "frontend/added.ts",
        "frontend/source.ts",
    )


def test_staged_snapshot_holds_only_the_pack_index_at_its_root(tmp_path: Path) -> None:
    """Materialize the pack's indexed files as the snapshot root, without siblings."""

    pack = _monorepo(tmp_path)
    (pack / "frontend/source.ts").write_text(
        "export const value = 2;", encoding="utf-8"
    )
    _git(pack.parent, "add", ".")
    (pack / "frontend/source.ts").write_text(
        "export const value = 3;", encoding="utf-8"
    )
    with repository_snapshot(pack, staged=True, package_name="Pack") as snapshot:
        assert snapshot.name == "Pack"
        assert (snapshot / "frontend/source.ts").read_text(
            encoding="utf-8"
        ) == "export const value = 2;"
        assert not (snapshot / "Sibling").exists()
        assert not (snapshot / "Pack").exists()
        assert sorted(
            path.relative_to(snapshot).as_posix()
            for path in snapshot.rglob("*")
            if path.is_file()
        ) == [".gitignore", "frontend/source.ts"]


def test_visible_sources_filter_the_nested_pack_by_git(tmp_path: Path) -> None:
    """Keep ignored output out of the pack inventory even without a local .git entry."""

    pack = _monorepo(tmp_path)
    (pack / "ignored").mkdir()
    (pack / "ignored/output.ts").write_text("export {};", encoding="utf-8")
    (pack / "frontend/new.ts").write_text("export {};", encoding="utf-8")
    assert visible_repository_paths(pack) == frozenset(
        {".gitignore", "frontend/source.ts", "frontend/new.ts"}
    )


def test_untracked_folders_inside_a_repository_are_not_filtered(tmp_path: Path) -> None:
    """Scan an exported snapshot completely even when it sits inside another repository."""

    repository = tmp_path / "outer"
    repository.mkdir()
    (repository / "tracked.txt").write_text("tracked", encoding="utf-8")
    _git(repository, "init", "--quiet")
    _git(repository, "add", ".")
    _git(repository, "commit", "--quiet", "-m", "fixture")
    snapshot = repository / "snapshot"
    snapshot.mkdir()
    (snapshot / "source.ts").write_text("export {};", encoding="utf-8")
    assert visible_repository_paths(snapshot) is None
