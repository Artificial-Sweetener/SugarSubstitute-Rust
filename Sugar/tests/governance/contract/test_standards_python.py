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
"""Static standards coverage for Python runtime code."""

from __future__ import annotations

from typing import Any

import importlib.util
from pathlib import Path
import pytest


def _load_audit_module() -> Any:
    """Load the standards audit script as a reusable test helper."""

    root = Path(__file__).resolve().parents[3]
    module_path = root / "scripts" / "standards_audit.py"
    spec = importlib.util.spec_from_file_location("standards_audit", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_python_standards_audit_passes() -> None:
    """Keep Python exception and docstring policy regressions out of the repo."""

    audit_module = _load_audit_module()
    failures = audit_module.run_audit()
    assert failures == []


@pytest.mark.parametrize(
    ("diagnostic", "mapped_error", "accepted"),
    [
        (
            "_logger.exception('integration failed')",
            "ExecutionNodeResolutionError",
            True,
        ),
        ("pass", "ExecutionNodeResolutionError", False),
        ("_logger.exception('integration failed')", "RuntimeError", False),
    ],
)
def test_execution_boundary_requires_diagnostics_and_typed_failure(
    tmp_path: Path, diagnostic: str, mapped_error: str, accepted: bool
) -> None:
    """Allow logged execution-boundary mapping without admitting silent catches."""
    audit = _load_audit_module()
    for relative in audit.DOCSTRING_TARGETS:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text('"""Audit fixture."""\n', encoding="utf-8")
    runtime = tmp_path / "sugarcubes" / "boundary.py"
    runtime.write_text(
        '"""Audit fixture for external callback failure."""\n'
        "def resolve():\n"
        "    try:\n"
        "        external_callback()\n"
        "    except Exception as error:\n"
        f"        {diagnostic}\n"
        f"        raise {mapped_error}('rejected') from error\n",
        encoding="utf-8",
    )
    failures = audit.run_audit(tmp_path)
    assert (failures == []) is accepted


@pytest.mark.parametrize(
    ("source", "rejected"),
    [
        ("subprocess.run(['git'], timeout=10)", True),
        ("process.communicate(timeout=5)", True),
        ("event.wait(5)", True),
        ("event.wait(timeout=0.5)", True),
        ("thread.join(timeout=2)", True),
        ("thread.join(2)", True),
        ("future.result(timeout=5)", True),
        ("asyncio.wait_for(task, 3)", True),
        ("asyncio.wait_for(task, timeout=3)", True),
        ("assert time.monotonic() - began < 1", True),
        ("assert perf_counter() < deadline", True),
        ("event.wait()", False),
        ("updates.wait(0)", False),
        ("fixture.result(1)", False),
        ("', '.join(parts)", False),
        ("wait_for_event(event)", False),
        ("asyncio.wait_for(task, HANG_GUARD_SECONDS)", False),
        ("TransportSession(response, expected_timeout=7200)", False),
        ("run.assert_called_once_with(['x'], timeout=10)", False),
        ("assert stamp == clock.now", False),
    ],
)
def test_wall_clock_waits_are_rejected_in_tests_and_tooling(
    tmp_path: Path, source: str, rejected: bool
) -> None:
    """No test or tool may pass or fail on elapsed seconds (decision M-3)."""

    audit_module = _load_audit_module()
    for root in audit_module.WALL_CLOCK_ROOTS:
        (tmp_path / root).mkdir()
    (tmp_path / "tests" / "test_case.py").write_text(source + "\n", encoding="utf-8")
    failures = audit_module._audit_wall_clock_waits(tmp_path)
    assert bool(failures) is rejected, failures


def test_wall_clock_rule_covers_tools_and_scripts(tmp_path: Path) -> None:
    """The rule reads every Python file under tests, tools and scripts."""

    audit_module = _load_audit_module()
    for root in audit_module.WALL_CLOCK_ROOTS:
        (tmp_path / root / "nested").mkdir(parents=True)
        (tmp_path / root / "nested" / "case.py").write_text(
            "subprocess.run(['x'], timeout=60)\n", encoding="utf-8"
        )
    (tmp_path / "sugarcubes").mkdir()
    (tmp_path / "sugarcubes" / "runtime.py").write_text(
        "urlopen(request, timeout=20)\n", encoding="utf-8"
    )
    failures = audit_module._audit_wall_clock_waits(tmp_path)
    assert sorted(failure.split(":")[0] for failure in failures) == [
        "scripts/nested/case.py",
        "tests/nested/case.py",
        "tools/nested/case.py",
    ]
