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
"""Prove exact, monotonic, Git-aware behavioral test selection."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tools.testing import runner as test_runner
from tools.testing.model import TestArea as _TestArea
from tools.testing.model import TestPolicy as _TestPolicy
from tools.testing.policy import load_test_policy
from tools.testing.selection import (
    TestSelectionError as _TestSelectionError,
    changed_paths,
    select_paths,
    validate_inventory,
)

PROJECT_ROOT = Path(__file__).resolve().parents[3]


@pytest.fixture(scope="module")
def policy() -> _TestPolicy:
    """Load the authoritative SugarCubes test policy once."""

    return load_test_policy(PROJECT_ROOT / "TEST_POLICY.toml")


def _group_names(policy: _TestPolicy, *paths: str) -> frozenset[str]:
    """Return stable group names selected for fixture paths."""

    return frozenset(group.name for group in select_paths(policy, paths).groups)


def _run_git(root: Path, *args: str) -> None:
    """Run one deterministic Git fixture command."""

    subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )


def test_current_repository_test_inventory_is_owned(policy: _TestPolicy) -> None:
    """Require every authored source and test module to have one policy owner."""

    validate_inventory(PROJECT_ROOT, policy)


def test_contention_sensitive_integration_proofs_are_process_isolated(
    policy: _TestPolicy,
) -> None:
    """Keep resource-heavy browser and authoring suites out of shared workers."""

    assert policy.area("authoring").isolated_proofs == frozenset({"integration"})
    assert policy.area("browser").isolated_proofs == frozenset({"integration"})


def test_cube_execution_program_has_independently_runnable_areas(
    policy: _TestPolicy,
) -> None:
    """Keep each architecture migration lane selectable during focused work."""

    assert {
        "workflow",
        "language",
        "execution",
        "optimizer",
        "library",
        "comfy_contract",
        "browser",
    } <= {area.name for area in policy.areas}


def test_phase_six_evidence_selects_governance_proofs(policy: _TestPolicy) -> None:
    """Keep machine-readable acceptance evidence inside test-policy ownership."""

    assert _group_names(
        policy,
        "artifacts/recipe-compatibility/fixture.semantic.json",
        "artifacts/phase6-acceptance/EVIDENCE.md",
    ) == frozenset({"governance/contract", "governance/unit"})


def test_canonical_workflow_changes_select_every_runtime_consumer(
    policy: _TestPolicy,
) -> None:
    """Fan canonical workflow changes out to every execution-facing consumer."""

    groups = _group_names(policy, "sugarcubes/workflow/reader.py")

    assert {
        "workflow/contract",
        "workflow/unit",
        "language/contract",
        "library/integration",
        "comfy_contract/contract",
        "execution/integration",
        "browser/integration",
    } <= groups


def test_source_change_selects_owner_and_boundary_subscribers(
    policy: _TestPolicy,
) -> None:
    """Fan a cube-document change out to every declared consumer proof."""

    groups = _group_names(policy, "sugarcubes/cube_model/schema.py")

    assert {
        "cube_model/contract",
        "cube_model/unit",
        "export/contract",
        "import/integration",
        "authoring/integration",
        "execution/integration",
    } <= groups


def test_windows_and_posix_paths_select_identical_groups(policy: _TestPolicy) -> None:
    """Normalize Windows separators without damaging dot-prefixed paths."""

    posix = select_paths(policy, (".github/workflows/quality.yml",))
    windows = select_paths(policy, (r".github\workflows\quality.yml",))

    assert windows.groups == posix.groups
    assert {group.name for group in posix.groups} == {
        "governance/contract",
        "governance/unit",
    }


def test_generated_browser_output_selects_its_authored_owner(
    policy: _TestPolicy,
) -> None:
    """Derive proof for compiler-owned JavaScript from authoritative TypeScript."""

    generated = select_paths(
        policy,
        ("web/comfyui/ui/core/FinalizedDefinition.js",),
    )
    authored = select_paths(
        policy,
        ("frontend/comfyui/ui/core/FinalizedDefinition.ts",),
    )

    assert generated.groups == authored.groups
    assert any(
        "generated output derives" in reason.reason for reason in generated.reasons
    )


def test_changed_test_selects_only_its_owned_group(policy: _TestPolicy) -> None:
    """Keep direct test iteration scoped to the test module's proof group."""

    groups = _group_names(
        policy,
        "tests/execution/unit/cube_prompt_pipeline.test.ts",
    )

    assert groups == frozenset({"execution/unit"})


def test_area_support_selects_every_area_proof(policy: _TestPolicy) -> None:
    """Treat shared area fixtures as dependencies of every local proof kind."""

    groups = _group_names(policy, "tests/backend_api/support/backend_fixtures.py")

    assert groups == frozenset({"backend_api/integration", "backend_api/unit"})


def test_area_package_selects_every_area_proof(policy: _TestPolicy) -> None:
    """Treat an area's package initialization as shared local test support."""

    groups = _group_names(policy, "tests/backend_api/__init__.py")

    assert groups == frozenset({"backend_api/integration", "backend_api/unit"})


def test_adding_paths_can_only_expand_selection(policy: _TestPolicy) -> None:
    """Keep required proof monotonic as a change's blast area grows."""

    first = _group_names(policy, "frontend/comfyui/ui/graph/DirtyManager.ts")
    expanded = _group_names(
        policy,
        "frontend/comfyui/ui/graph/DirtyManager.ts",
        "sugarcubes/cube_model/schema.py",
    )

    assert first <= expanded


def test_unknown_runtime_path_fails_closed(policy: _TestPolicy) -> None:
    """Never silently select zero tests for an unowned runtime source."""

    with pytest.raises(_TestSelectionError, match="not allowed to select zero tests"):
        select_paths(policy, ("new_runtime/owner.ts",))


def test_ambiguous_runtime_path_fails_closed(policy: _TestPolicy) -> None:
    """Require one authoritative area when source patterns overlap."""

    duplicate = _TestArea(
        name="duplicate",
        sources=("sugarcubes/cube_model/**",),
        proofs=("unit",),
        isolated_proofs=frozenset(),
    )
    ambiguous = _TestPolicy(
        schema=policy.schema,
        test_root=policy.test_root,
        areas=(*policy.areas, duplicate),
        boundaries=policy.boundaries,
        subscriptions=policy.subscriptions,
    )

    with pytest.raises(_TestSelectionError, match="multiple areas"):
        select_paths(ambiguous, ("sugarcubes/cube_model/schema.py",))


def test_commit_mode_expands_an_affected_product_completely(
    policy: _TestPolicy,
) -> None:
    """Expand a runtime impact to every SugarCubes proof before commit."""

    selection = select_paths(
        policy,
        ("frontend/comfyui/ui/graph/DirtyManager.ts",),
        commit=True,
    )

    assert selection.groups == tuple(sorted(policy.groups))


def test_staged_path_query_reads_exact_git_index(tmp_path: Path) -> None:
    """Select staged changes independently from later worktree edits."""

    _run_git(tmp_path, "init", "--quiet")
    source = tmp_path / "source.py"
    source.write_text("value = 1\n", encoding="utf-8")
    _run_git(tmp_path, "add", "source.py")
    source.write_text("value = 2\n", encoding="utf-8")
    (tmp_path / "untracked.py").write_text("value = 3\n", encoding="utf-8")

    assert changed_paths(tmp_path, staged=True) == ("source.py",)


def test_isolated_gate_runs_each_group_in_a_fresh_process(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep cross-group process state out of the diagnostic isolation gate."""

    area = _TestArea(
        name="feature",
        sources=("src/**",),
        proofs=("unit", "contract"),
        isolated_proofs=frozenset(),
    )
    policy = _TestPolicy(
        schema=1,
        test_root="tests",
        areas=(area,),
        boundaries=(),
        subscriptions=(),
    )
    for proof in area.proofs:
        proof_root = tmp_path / "tests" / "feature" / proof
        proof_root.mkdir(parents=True)
        (proof_root / f"test_{proof}.py").write_text(
            "def test_placeholder() -> None:\n    pass\n",
            encoding="utf-8",
        )
    commands: list[tuple[str, ...]] = []

    def capture(_root: Path, command: list[str]) -> int:
        """Capture one isolated subprocess command."""

        commands.append(tuple(command))
        return 0

    monkeypatch.setattr(test_runner, "_run", capture)

    assert test_runner.run_isolated_groups(tmp_path, policy) == 0
    assert len(commands) == 2
    assert commands[0] != commands[1]
