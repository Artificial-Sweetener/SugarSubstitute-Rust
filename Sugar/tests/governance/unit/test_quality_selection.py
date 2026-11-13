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
"""Prove selective checks, shared fan-out and exact staged execution foundations."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tools.quality.plan import load_checks, select_checks
from tools.quality.__main__ import main as quality_main
from tools.quality.snapshot import quality_snapshot
from tools.testing.model import TestPolicy as _TestPolicy
from tools.testing.policy import load_test_policy
from tools.testing.selection import changed_paths, select_paths

ROOT = Path(__file__).resolve().parents[3]


def _checks(*paths: str) -> frozenset[str]:
    """Derive quality profiles from real behavioral ownership and subscriptions."""

    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    return frozenset(
        check.name
        for check in select_checks(
            load_checks(ROOT, policy), select_paths(policy, paths)
        )
    )


def test_unrelated_work_omits_editor_checks_and_shared_work_includes_consumers() -> (
    None
):
    """Keep editor-only work isolated and validate browser consumers of shared contracts."""

    assert _checks("sugarcubes/cube_model/schema.py") == {"comfy"}
    assert _checks("scripts/build-web.ts") == {"comfy"}
    assert _checks("integrations/photoshop/src/index.ts") == {
        "integrations",
        "photoshop",
    }
    assert _checks("integrations/contracts/panel.schema.json") == {
        "comfy",
        "integrations",
        "photoshop",
        "gimp",
    }
    assert _checks("integrations/shared/PanelSession.ts") == {
        "comfy",
        "integrations",
        "photoshop",
        "gimp",
    }
    assert _checks("scripts/photoshop/build.ts") == {"integrations", "photoshop"}
    assert _checks("package-lock.json") == {
        "comfy",
        "integrations",
        "photoshop",
        "gimp",
    }
    assert _checks("integrations/gimp/companion/src/connection.rs") == {
        "integrations",
        "gimp",
    }
    assert _checks("integrations/gimp/native/images.py") == {
        "integrations",
        "gimp",
    }


def test_check_selection_is_monotonic_and_common_config_selects_every_proof() -> None:
    """Prevent later paths from narrowing checks or shared tooling from skipping consumers."""

    assert _checks("integrations/photoshop/src/index.ts") <= _checks(
        "integrations/photoshop/src/index.ts", "sugarcubes/cube_model/schema.py"
    )
    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    assert select_paths(policy, ("QUALITY_POLICY.toml",)).groups == tuple(
        sorted(policy.groups)
    )


def test_quality_policy_rejects_unknown_fields_and_uncovered_areas(
    tmp_path: Path,
) -> None:
    """Fail closed when validation profiles drift from behavioral ownership."""

    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    (tmp_path / "package.json").write_text(
        '{"scripts":{"check:fixture":"fixture"}}', encoding="utf-8"
    )
    (tmp_path / "QUALITY_POLICY.toml").write_text(
        'schema=1\n[[check]]\nname="fixture"\nscript="check:fixture"\nareas=["photoshop"]\nexclude_areas=[]\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="lack quality checks"):
        load_checks(tmp_path, policy)
    (tmp_path / "QUALITY_POLICY.toml").write_text(
        'schema=1\n[[check]]\nname="fixture"\nscript="check:fixture"\nareas=["*"]\nexclude_areas=[]\nunknown=true\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="Unknown or missing"):
        load_checks(tmp_path, policy)


def _git(root: Path, *arguments: str) -> None:
    """Run fixture Git operations without ambient user configuration."""

    subprocess.run(["git", *arguments], cwd=root, check=True, capture_output=True)


def test_staged_snapshot_uses_index_bytes_and_preserves_dependency_tree(
    tmp_path: Path,
) -> None:
    """Prevent a later worktree edit or snapshot cleanup from changing staged validation."""

    _git(tmp_path, "init", "--quiet")
    source = tmp_path / "source.ts"
    source.write_text("export const value = 'indexed';", encoding="utf-8")
    _git(tmp_path, "add", "source.ts")
    source.write_text("export const value = 'worktree';", encoding="utf-8")
    dependencies = tmp_path / "node_modules"
    dependencies.mkdir()
    (dependencies / "keep.txt").write_text("installed", encoding="utf-8")
    with quality_snapshot(tmp_path, staged=True) as snapshot:
        assert (snapshot / "source.ts").read_text(
            encoding="utf-8"
        ) == "export const value = 'indexed';"
        assert (snapshot / "node_modules/keep.txt").read_text(
            encoding="utf-8"
        ) == "installed"
    assert (dependencies / "keep.txt").read_text(encoding="utf-8") == "installed"


def test_rename_selection_keeps_both_old_and_new_owners(tmp_path: Path) -> None:
    """Preserve source removal impact when a file moves between integrations."""

    _git(tmp_path, "init", "--quiet")
    (tmp_path / "source.ts").write_text("export const value=1;", encoding="utf-8")
    _git(tmp_path, "add", "source.ts")
    _git(
        tmp_path,
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "fixture",
    )
    _git(tmp_path, "mv", "source.ts", "destination.ts")
    assert set(changed_paths(tmp_path, staged=True)) == {"source.ts", "destination.ts"}


def test_unregistered_editor_cannot_select_zero_proofs() -> None:
    """Require a new host to declare test ownership before implementation."""

    policy: _TestPolicy = load_test_policy(ROOT / "TEST_POLICY.toml")
    with pytest.raises(ValueError, match="not allowed to select zero tests"):
        select_paths(policy, ("integrations/new_editor/src/index.ts",))


def test_staged_gate_executes_indexed_checks_and_tests(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exercise the complete staged runner with worktree source that would fail validation."""

    _git(tmp_path, "init", "--quiet")
    files = {
        "value.py": "value: int = 1\n",
        "package.json": '{"scripts":{"verify":"node verify.cjs"}}',
        "verify.cjs": "const fs=require('node:fs');if(process.env.GIT_INDEX_FILE || !fs.readFileSync('value.py','utf8').includes('1'))process.exit(1);",
        "QUALITY_POLICY.toml": 'schema=1\n[[check]]\nname="fixture"\nscript="verify"\nareas=["*"]\nexclude_areas=[]\n',
        "TEST_POLICY.toml": 'schema=1\ntest_root="tests"\nboundary=[]\nsubscription=[]\n[[area]]\nname="fixture"\nsources=["value.py", "*.json", "*.toml", "*.cjs"]\nproofs=["unit"]\n',
        "ARCHITECTURE_POLICY.toml": 'schema=1\ndependency=[]\n[structure]\nsource_roots=["value.py"]\nextensions=[".py"]\nexcluded_prefixes=["node_modules/"]\nsoft_lines=350\nhard_lines=500\n',
        "ARCHITECTURE_DEBT.toml": "schema=1\ndebt=[]\n",
        "ARCHITECTURE_WAIVERS.toml": "schema=1\nwaiver=[]\n",
        "tests/fixture/unit/test_indexed.py": "import os\nfrom value import value\n\ndef test_indexed() -> None:\n    assert value == 1\n    assert 'GIT_INDEX_FILE' not in os.environ\n",
    }
    for relative, content in files.items():
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    _git(tmp_path, "add", ".")
    (tmp_path / "value.py").write_text("value: int = 2\n", encoding="utf-8")
    (tmp_path / "node_modules").mkdir()
    monkeypatch.setenv("GIT_INDEX_FILE", str(tmp_path / ".git/index"))
    assert quality_main(("staged",), project_root=tmp_path) == 0
    assert (tmp_path / "value.py").read_text(encoding="utf-8") == "value: int = 2\n"


def test_shared_headless_browser_fixture_selects_every_proof_and_quality_profile() -> (
    None
):
    """Recheck every browser consumer when its shared launch or teardown policy changes."""
    policy = load_test_policy(ROOT / "TEST_POLICY.toml")
    path = "tests/frontend_core/support/BrowserFixtureLifetime.ts"
    selection = select_paths(policy, (path,))
    assert set(selection.groups) == set(policy.groups)
    assert _checks(path) == {"comfy", "integrations", "photoshop", "gimp"}
