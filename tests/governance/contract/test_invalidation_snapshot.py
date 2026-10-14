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
"""Prove observer governance reads source and ownership from the same Git index."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tools.architecture.browser_ui_policy import scan_browser_ui
from tools.architecture.registry import load_policy
from tools.architecture.snapshot import repository_snapshot

PROJECT_ROOT = Path(__file__).resolve().parents[3]


def test_staged_observer_ownership_is_not_weakened_by_unstaged_policy(
    tmp_path: Path,
) -> None:
    """Reject an indexed observer even when worktree source and owner policy permit it."""
    (tmp_path / "frontend").mkdir()
    source = tmp_path / "frontend/feature.ts"
    source.write_text(
        "const Watch = window.MutationObserver; new Watch(() => {});", encoding="utf-8"
    )
    policy = tmp_path / "ARCHITECTURE_POLICY.toml"
    contents = """schema = 1
dependency = []
[invalidation]
mutation_observer_owners = ["frontend/approved.ts"]
[structure]
source_roots = ["frontend"]
extensions = [".ts"]
excluded_prefixes = ["vendor/"]
soft_lines = 350
hard_lines = 500
"""
    policy.write_text(contents, encoding="utf-8")
    for args in [("init",), ("add", ".")]:
        subprocess.run(
            ["git", *args], cwd=tmp_path, check=True, capture_output=True, timeout=10
        )
    policy.write_text(contents.replace("approved.ts", "feature.ts"), encoding="utf-8")
    source.write_text("export const harmless = true;", encoding="utf-8")
    with repository_snapshot(tmp_path, staged=True) as snapshot:
        owners = load_policy(
            snapshot / "ARCHITECTURE_POLICY.toml"
        ).mutation_observer_owners
        diagnostics = scan_browser_ui(
            snapshot, PROJECT_ROOT, ("frontend/feature.ts",), owners
        )
    assert len(diagnostics) == 1
    assert diagnostics[0].rule == "UI_OBSERVER_OWNER"


def test_missing_compiler_runtime_fails_closed(tmp_path: Path) -> None:
    """Keep the architecture gate blocking when typed observation checks cannot run."""
    diagnostics = scan_browser_ui(tmp_path, tmp_path, ("frontend/feature.ts",), ())
    assert diagnostics[0].rule == "UI_AUDIT_FAILURE"
