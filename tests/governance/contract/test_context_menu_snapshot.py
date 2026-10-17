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
"""Prove menu ownership comes from the exact Git index rather than unstaged presentation changes."""

import subprocess
from pathlib import Path

from tools.architecture.browser_ui_policy import scan_browser_ui
from tools.architecture.registry import load_policy
from tools.architecture.snapshot import repository_snapshot

PROJECT_ROOT = Path(__file__).resolve().parents[3]


def test_staged_menu_renderer_cannot_be_hidden_by_worktree_policy(
    tmp_path: Path,
) -> None:
    """Reject a staged menu renderer even when its worktree role and owner are changed."""
    (tmp_path / "frontend").mkdir()
    source = tmp_path / "frontend/feature.ts"
    source.write_text('document.body.setAttribute("role", "menu");', encoding="utf-8")
    policy = tmp_path / "ARCHITECTURE_POLICY.toml"
    contents = """schema = 1
dependency = []
[menus]
context_menu = "frontend/approved.ts"
floating_surface = "frontend/floating.ts"
control_runtime = "frontend/fluent.ts"
search_dialogs = []
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
    source.write_text('document.body.setAttribute("role", "button");', encoding="utf-8")
    with repository_snapshot(tmp_path, staged=True) as snapshot:
        menus = load_policy(snapshot / "ARCHITECTURE_POLICY.toml").menus
        diagnostics = scan_browser_ui(
            snapshot, PROJECT_ROOT, ("frontend/feature.ts",), (), menus=menus
        )
    assert any(item.rule == "UI_CONTEXT_MENU_RENDERER" for item in diagnostics)
