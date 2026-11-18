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
"""Verify that repository quality gates never rewrite the worktree."""

from __future__ import annotations

import json
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[3]


def test_check_script_uses_validation_only_formatters() -> None:
    """Keep the authoritative quality gate deterministic and non-mutating."""

    package = json.loads((PROJECT_ROOT / "package.json").read_text(encoding="utf-8"))
    scripts = package["scripts"]

    assert "npm run format:check" in scripts["check"]
    assert "npm run architecture" in scripts["check"]
    assert "npm run format" not in scripts["check"].replace("npm run format:check", "")
    assert "--check" in scripts["format:check"]
    assert r"\.venv" in scripts["format:check"]
    assert r"\.venv" in scripts["format"]
    assert "prettier --check" in scripts["format:check"]
    assert "--write" not in scripts["check"]
    assert "prettier --write" in scripts["format"]
    assert scripts["check:commit"] == "npm run check:staged"
    assert scripts["check:changed"] == "python -m tools.quality changed"
    assert scripts["check:staged"] == "python -m tools.quality staged"
    assert scripts["test:changed"] == "python -m tools.test changed"
    assert scripts["test:commit"] == "python -m tools.test staged"
    assert scripts["test:isolated"] == "python -m tools.test isolated"
    assert scripts["test:all"] == "python -m tools.test all"
    assert "npm run test:all" in scripts["check"]


def _steps(scripts: dict[str, str], name: str) -> list[str]:
    """Expand one npm script that only chains other npm scripts into the steps it runs."""

    parts = scripts[name].split(" && ")
    if not all(part.startswith("npm run ") for part in parts):
        return [name]
    return [
        step
        for part in parts
        for step in _steps(scripts, part.removeprefix("npm run "))
    ]


def test_the_comfy_profile_runs_exactly_the_steps_of_the_comfy_check() -> None:
    """Keep the steps the gate runs side by side equal to the chain a person runs by hand."""

    import tomllib

    package = json.loads((PROJECT_ROOT / "package.json").read_text(encoding="utf-8"))
    policy = tomllib.loads(
        (PROJECT_ROOT / "QUALITY_POLICY.toml").read_text(encoding="utf-8")
    )
    comfy = next(check for check in policy["check"] if check["name"] == "comfy")

    assert comfy["scripts"] == _steps(package["scripts"], "check:comfy")
    # The complete gate runs every one of those steps too.
    assert set(comfy["scripts"]) <= set(_steps(package["scripts"], "check"))
