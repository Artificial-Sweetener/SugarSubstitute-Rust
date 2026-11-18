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
"""Prove the gate runs its processes side by side within a budget and loses no failure."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

from tools.testing.model import TestGroup as _TestGroup
from tools.testing.model import TestSelection as _TestSelection
from tools.testing.parallel import (
    DEFAULT_JOBS,
    JOBS_VARIABLE,
    Command,
    gate_jobs,
    run_commands,
)
from tools.testing.policy import load_test_policy
from tools.testing.runner import selection_commands

ROOT = Path(__file__).resolve().parents[3]


def _python(label: str, root: Path, code: str, weight: int = 1) -> Command:
    """Describe one short Python process."""

    return Command(label, root, (sys.executable, "-c", code), weight)


def _spans(root: Path, name: str) -> tuple[float, float]:
    """Read when a recording process started and ended."""

    started, ended = (root / name).read_text(encoding="utf-8").split()
    return float(started), float(ended)


def _recording(name: str, seconds: float) -> str:
    """Write the process's own start and end times, `seconds` apart, to `name`."""

    return (
        "import time, pathlib; s = time.time(); "
        f"time.sleep({seconds}); "
        f"pathlib.Path({name!r}).write_text(f'{{s}} {{time.time()}}')"
    )


def test_processes_overlap_within_the_budget_and_never_beyond_it(
    tmp_path: Path,
) -> None:
    """Two units at once: the first two overlap, and the third waits for a free unit."""

    commands = [
        _python(name, tmp_path, _recording(name, 0.6)) for name in ("a", "b", "c")
    ]
    assert run_commands(commands, jobs=2) == 0
    a, b, c = (_spans(tmp_path, name) for name in ("a", "b", "c"))
    assert max(a[0], b[0]) < min(a[1], b[1]), "the first two ran at the same time"
    assert c[0] >= min(a[1], b[1]), "the third started only once a unit was free"


def test_a_weighted_process_occupies_its_share_and_an_oversized_one_runs_alone(
    tmp_path: Path,
) -> None:
    """A process that brings its own workers leaves room only for what still fits."""

    commands = [
        _python("heavy", tmp_path, _recording("heavy", 0.6), weight=2),
        _python("light", tmp_path, _recording("light", 0.2)),
        _python("huge", tmp_path, _recording("huge", 0.2), weight=99),
    ]
    assert run_commands(commands, jobs=2) == 0
    heavy, light, huge = (_spans(tmp_path, name) for name in ("heavy", "light", "huge"))
    assert (
        light[0] >= heavy[1]
    ), "nothing fits beside a process holding the whole budget"
    assert huge[0] >= light[1], "an oversized process waits for the whole budget"


def test_every_reached_failure_is_shown_whole_and_the_first_in_order_is_returned(
    tmp_path: Path,
    capfdbinary: pytest.CaptureFixture[bytes],
) -> None:
    """Concurrent output never interleaves, and a slow early failure outranks a fast later one."""

    commands = [
        _python(
            "slow failure",
            tmp_path,
            "import sys, time; time.sleep(0.5); print('slow line 1'); print('slow line 2'); "
            "sys.exit(3)",
        ),
        _python(
            "fast failure", tmp_path, "import sys; print('fast line'); sys.exit(5)"
        ),
        _python("success", tmp_path, "print('fine')"),
    ]
    assert run_commands(commands, jobs=3) == 3
    printed = capfdbinary.readouterr().out.decode().replace("\r\n", "\n")
    assert "===== slow failure: FAILED with status 3" in printed
    assert "===== fast failure: FAILED with status 5" in printed
    assert "===== success: passed" in printed
    assert "slow line 1\nslow line 2\n" in printed


def test_nothing_new_starts_after_a_failure(tmp_path: Path) -> None:
    """A failed gate stops admitting work instead of running everything that remains."""

    commands = [
        _python("failure", tmp_path, "import sys; sys.exit(1)"),
        _python("never", tmp_path, _recording("never", 0)),
    ]
    assert run_commands(commands, jobs=1) == 1
    assert not (tmp_path / "never").exists()


def test_a_process_that_cannot_start_fails_the_run(tmp_path: Path) -> None:
    """A missing executable is a failure with a diagnostic, never a silent pass."""

    missing = Command("missing", tmp_path, ("sugarcubes-no-such-executable",))
    assert run_commands([missing], jobs=1) == 127
    assert run_commands([], jobs=1) == 0
    with pytest.raises(ValueError, match="at least one"):
        run_commands([missing], jobs=0)


def test_the_budget_has_a_default_and_refuses_a_meaningless_override() -> None:
    """A mistyped override must not silently become an unbounded or zero budget."""

    assert gate_jobs({}) == DEFAULT_JOBS
    assert gate_jobs({JOBS_VARIABLE: " "}) == DEFAULT_JOBS
    assert gate_jobs({JOBS_VARIABLE: "3"}) == 3
    for value in ("0", "-2", "many", "1.5"):
        with pytest.raises(ValueError, match="positive whole number"):
            gate_jobs({JOBS_VARIABLE: value})


def test_a_selection_runs_shared_proofs_together_and_isolated_modules_apart() -> None:
    """The processes cover exactly the selected modules, each isolated module on its own."""

    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    selection = _TestSelection(
        groups=(
            _TestGroup("workflow", "unit"),
            _TestGroup("workflow", "integration"),
            _TestGroup("prompt_editor", "integration"),
        ),
        reasons=(),
    )
    commands = selection_commands(ROOT, policy, selection, jobs=8)
    shared = [command for command in commands if command.label == "TypeScript proofs"]
    assert len(shared) == 1
    assert "--maxWorkers=4" in shared[0].arguments and shared[0].weight == 4
    assert any(
        argument.endswith("desktop_runtime_composition.test.ts")
        for argument in shared[0].arguments
    )
    python = [command for command in commands if command.label == "Python proofs"]
    assert len(python) == 1 and python[0].weight == 1
    isolated = [command for command in commands if "prompt_editor" in command.label]
    modules = sorted(
        path.relative_to(ROOT)
        for path in (ROOT / "tests/prompt_editor/integration").rglob("*.test.ts")
    )
    assert [command.label for command in isolated] == [str(path) for path in modules]
    assert all(
        "--runInBand" in command.arguments and command.weight == 1
        for command in isolated
    )
    assert not any(
        "prompt_editor" in argument for argument in shared[0].arguments
    ), "an isolated module never joins the shared run"


def test_the_same_selection_always_lists_the_same_processes() -> None:
    """Which failure a run returns first must not depend on how the run happened to go."""

    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    selection = _TestSelection(groups=policy.groups, reasons=())
    first = selection_commands(ROOT, policy, selection, jobs=8)
    assert first == selection_commands(ROOT, policy, selection, jobs=8)
