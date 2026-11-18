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
"""Execute selected Python, TypeScript and Rust proof groups side by side, deterministically."""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .model import TestGroup, TestPolicy, TestSelection
from .parallel import Command, gate_jobs, run_commands
from .rust import rust_test_targets

_JEST = ("node", "--experimental-vm-modules", "node_modules/jest/bin/jest.js")


def run_selection(
    root: Path,
    policy: TestPolicy,
    selection: TestSelection,
    *,
    environment: Mapping[str, str] | None = None,
    alongside: Sequence[Command] = (),
) -> int:
    """Run the selected groups, and any `alongside` validation, within one bounded budget.

    Returns the first failing process status in a fixed order: `alongside` first, then the
    selection's own processes as `selection_commands` lists them.
    """

    jobs = gate_jobs(environment)
    return run_commands(
        (*alongside, *selection_commands(root, policy, selection, jobs=jobs)),
        jobs=jobs,
        environment=environment,
    )


def selection_commands(
    root: Path,
    policy: TestPolicy,
    selection: TestSelection,
    *,
    jobs: int,
) -> tuple[Command, ...]:
    """List one process per shared run and per isolated proof module, in a fixed order.

    Proofs that may share a process run together, as their runner schedules them; a module of a
    policy-isolated proof keeps a fresh process of its own. A runner that fans out its own
    workers is told how many, and occupies that much of the budget.
    """

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
    workers = max(1, jobs // 2)
    commands: list[Command] = [
        *_path_commands(
            "TypeScript proofs",
            root,
            (*_JEST, f"--maxWorkers={workers}", "--runTestsByPath"),
            ordinary_typescript,
            weight=workers,
        ),
        *_path_commands(
            "Python proofs",
            root,
            (sys.executable, "-m", "pytest", "-q"),
            ordinary_python,
            weight=1,
        ),
    ]
    for path in sorted(isolated):
        relative = str(path.relative_to(root))
        if path.suffix == ".py":
            commands.append(
                Command(
                    relative, root, (sys.executable, "-m", "pytest", "-q", relative)
                )
            )
        elif path.suffix == ".ts":
            commands.append(
                Command(
                    relative,
                    root,
                    (*_JEST, "--runInBand", "--runTestsByPath", relative),
                )
            )
        else:
            ordinary_rust.append(path)
    # A native build uses every core it finds, so each one takes the whole budget.
    commands.extend(
        Command(
            f"Rust proof {target.name}",
            target.manifest.parent,
            (*target.runner, "--test", target.name),
            weight=jobs,
        )
        for target in rust_test_targets(root, tuple(ordinary_rust))
    )
    return tuple(commands)


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


def _path_commands(
    label: str,
    root: Path,
    prefix: tuple[str, ...],
    paths: Sequence[str],
    *,
    weight: int,
) -> tuple[Command, ...]:
    """Preserve selected paths while keeping each command below Windows' CreateProcess limit."""

    batches: list[list[str]] = []
    batch: list[str] = []
    for path in paths:
        if len(subprocess.list2cmdline([*prefix, *batch, path])) >= 30000:
            if not batch:
                raise ValueError(
                    "A selected test path exceeds the subprocess argument budget."
                )
            batches.append(batch)
            batch = [path]
        else:
            batch.append(path)
    if batch:
        batches.append(batch)
    return tuple(
        Command(
            label if len(batches) == 1 else f"{label} {number} of {len(batches)}",
            root,
            (*prefix, *selected),
            weight=weight,
        )
        for number, selected in enumerate(batches, start=1)
    )


__all__ = [
    "group_test_files",
    "run_isolated_groups",
    "run_selection",
    "selection_commands",
]
