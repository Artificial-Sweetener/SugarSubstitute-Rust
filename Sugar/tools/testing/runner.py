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
"""Execute selected Python, TypeScript and Rust proof groups deterministically."""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

from .model import TestGroup, TestPolicy, TestSelection
from .rust import rust_test_targets


def run_selection(
    root: Path,
    policy: TestPolicy,
    selection: TestSelection,
    *,
    environment: Mapping[str, str] | None = None,
) -> int:
    """Run selected groups and return the first failing process status."""

    ordinary_python: list[str] = []
    ordinary_typescript: list[str] = []
    ordinary_rust: list[Path] = []
    isolated: list[Path] = []
    for group in selection.groups:
        files = group_test_files(root, policy, group)
        area = policy.area(group.area)
        if group.proof in area.isolated_proofs:
            isolated.extend(files)
            continue
        ordinary_python.extend(
            str(path.relative_to(root)) for path in files if path.suffix == ".py"
        )
        ordinary_typescript.extend(
            str(path.relative_to(root)) for path in files if path.suffix == ".ts"
        )
        ordinary_rust.extend(path for path in files if path.suffix == ".rs")
    if ordinary_python:
        status = _run_paths(
            root,
            [sys.executable, "-m", "pytest", "-q"],
            ordinary_python,
            environment=environment,
        )
        if status:
            return status
    if ordinary_typescript:
        status = _run_paths(
            root,
            [
                "node",
                "--experimental-vm-modules",
                "node_modules/jest/bin/jest.js",
                "--runTestsByPath",
            ],
            ordinary_typescript,
            environment=environment,
        )
        if status:
            return status
    for target in rust_test_targets(root, tuple(ordinary_rust)):
        status = _run(
            target.manifest.parent,
            [
                *target.runner,
                "--test",
                target.name,
            ],
            environment=environment,
        )
        if status:
            return status
    for path in sorted(isolated):
        relative = str(path.relative_to(root))
        if path.suffix == ".py":
            status = _run(
                root,
                [sys.executable, "-m", "pytest", "-q", relative],
                environment=environment,
            )
        elif path.suffix == ".ts":
            status = _run(
                root,
                [
                    "node",
                    "--experimental-vm-modules",
                    "node_modules/jest/bin/jest.js",
                    "--runInBand",
                    "--runTestsByPath",
                    relative,
                ],
                environment=environment,
            )
        else:
            target = rust_test_targets(root, (path,))[0]
            status = _run(
                target.manifest.parent,
                [
                    *target.runner,
                    "--test",
                    target.name,
                ],
                environment=environment,
            )
        if status:
            return status
    return 0


def run_isolated_groups(root: Path, policy: TestPolicy) -> int:
    """Run every declared proof group in its own fresh test process."""

    for group in policy.groups:
        selection = TestSelection(groups=(group,), reasons=())
        status = run_selection(root, policy, selection)
        if status:
            return status
    return 0


def group_test_files(
    root: Path,
    policy: TestPolicy,
    group: TestGroup,
) -> tuple[Path, ...]:
    """Return every authored proof module in one selected group."""

    group_root = root / policy.test_root / group.area / group.proof
    return tuple(
        sorted(
            path
            for path in group_root.rglob("*")
            if path.is_file()
            and (
                (path.name.startswith("test_") and path.suffix == ".py")
                or path.name.endswith((".test.ts", ".spec.ts", "_test.rs"))
            )
        )
    )


def _run(
    root: Path, command: list[str], *, environment: Mapping[str, str] | None = None
) -> int:
    """Run one inherited-output test process without shell interpretation."""

    completed = subprocess.run(
        command, cwd=root, check=False, env=environment
    )  # noqa: S603
    return completed.returncode


def _run_paths(
    root: Path,
    prefix: list[str],
    paths: list[str],
    *,
    environment: Mapping[str, str] | None = None,
) -> int:
    """Preserve selected paths while keeping each command below Windows' CreateProcess limit."""

    command = prefix.copy()
    for path in paths:
        candidate = [*command, path]
        if len(subprocess.list2cmdline(candidate)) >= 30000:
            if len(command) == len(prefix):
                raise ValueError(
                    "A selected test path exceeds the subprocess argument budget."
                )
            status = _run(root, command, environment=environment)
            if status:
                return status
            command = [*prefix, path]
        else:
            command = candidate
    return (
        _run(root, command, environment=environment)
        if len(command) > len(prefix)
        else 0
    )


__all__ = ["group_test_files", "run_isolated_groups", "run_selection"]
