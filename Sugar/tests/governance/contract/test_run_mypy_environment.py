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
"""Prove the type-check gate refuses an interpreter that lacks the declared dependencies."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest
from packaging.requirements import Requirement

from scripts import run_mypy

_PROJECT_ROOT = Path(__file__).resolve().parents[3]


def _versions(installed: dict[str, str]) -> Callable[[str], str | None]:
    """Answer version queries from a fixed table of installed distributions."""

    return lambda name: installed.get(name)


def test_declared_requirements_cover_the_packages_mypy_resolves_types_from() -> None:
    """Runtime dependencies and the type-resolving development packages are required."""

    names = {
        r.name.lower()
        for r in run_mypy.declared_requirements(_PROJECT_ROOT / "pyproject.toml")
    }

    assert {"av", "pygit2", "pillow", "aiohttp", "numpy", "pytest", "mypy"} <= names
    assert names.isdisjoint({"black", "ruff"})


def test_a_missing_distribution_is_reported_as_not_installed() -> None:
    """A dependency the interpreter lacks cannot be type-checked."""

    result = run_mypy.unsatisfied_requirements(
        [Requirement("av>=16.0.0"), Requirement("certifi")],
        _versions({"certifi": "2025.4.26"}),
    )

    assert result == ["av>=16.0.0 (not installed)"]


def test_an_installed_distribution_older_than_the_declared_minimum_is_reported() -> (
    None
):
    """An older package carries older type information than the project is written against."""

    result = run_mypy.unsatisfied_requirements(
        [Requirement("Pillow>=11.3,<13")], _versions({"Pillow": "11.0.0"})
    )

    assert result == ["Pillow<13,>=11.3 (installed 11.0.0)"]


def test_pinned_and_in_range_distributions_are_accepted() -> None:
    """Exact pins and ranges are satisfied by matching installed versions."""

    result = run_mypy.unsatisfied_requirements(
        [
            Requirement("pygit2==1.20.1"),
            Requirement("Pillow>=11.3,<13"),
            Requirement("aiohttp"),
        ],
        _versions({"pygit2": "1.20.1", "Pillow": "12.3.0", "aiohttp": "3.13.5"}),
    )

    assert result == []


def test_main_stops_before_running_mypy_when_dependencies_are_missing(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """The gate fails once, with the install command, instead of reporting unfixable type errors."""

    monkeypatch.setattr(run_mypy, "installed_version", lambda _name: None)
    ran: list[list[str]] = []

    def record(arguments: list[str], *, cwd: Path, environment: dict[str, str]) -> int:
        """Remember each mypy invocation instead of starting a process."""

        ran.append(arguments)
        return 0

    monkeypatch.setattr(run_mypy, "_run_mypy", record)

    assert run_mypy.main() == 2
    assert ran == []
    message = capsys.readouterr().err
    assert "pygit2==1.20.1 (not installed)" in message
    assert 'python -m pip install -e ".[dev]"' in message
