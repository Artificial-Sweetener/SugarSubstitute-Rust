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
"""Verify executable architecture, debt, waiver, and snapshot governance."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tools.architecture.checker import check_repository
from tools.architecture.registry import RegistryError, load_waivers
from tools.architecture.registry import load_policy
from tools.architecture.scanner import source_fingerprint, source_paths
from tools.architecture.system_git_policy import scan_system_git_dependencies

PROJECT_ROOT = Path(__file__).resolve().parents[3]


def _write_policy(
    root: Path,
    *,
    dependencies: str = "",
    hard_lines: int = 5,
) -> None:
    """Write one minimal strict architecture policy fixture."""

    dependency_section = dependencies.strip() or "dependency = []"
    (root / "ARCHITECTURE_POLICY.toml").write_text(
        f"""schema = 1
{dependency_section}

[structure]
source_roots = ["src"]
extensions = [".py", ".ts"]
excluded_prefixes = ["vendor/"]
soft_lines = 3
hard_lines = {hard_lines}
""",
        encoding="utf-8",
    )


def _write_empty_registries(root: Path) -> None:
    """Write empty debt and waiver registries."""

    (root / "ARCHITECTURE_DEBT.toml").write_text(
        "schema = 1\ndebt = []\n", encoding="utf-8"
    )
    (root / "ARCHITECTURE_WAIVERS.toml").write_text(
        "schema = 1\nwaiver = []\n", encoding="utf-8"
    )


def _initialize_fixture(root: Path, source: str = "value = 1\n") -> Path:
    """Create one minimal governed repository fixture."""

    source_root = root / "src"
    source_root.mkdir()
    source_path = source_root / "domain.py"
    source_path.write_text(source, encoding="utf-8")
    _write_policy(root)
    _write_empty_registries(root)
    return source_path


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


def test_current_repository_architecture_is_governed() -> None:
    """Keep the real current repository policy, debt, and waivers valid."""

    result = check_repository(PROJECT_ROOT)

    assert result.succeeded, tuple(item.render() for item in result.diagnostics)
    assert all(item.rule == "STRUCT002" for item in result.diagnostics)


def test_debt_fingerprint_changes_with_exact_source_state(tmp_path: Path) -> None:
    """Bind debt identity to normalized path and exact source content."""

    source_path = _initialize_fixture(tmp_path)
    first = source_fingerprint(tmp_path, ("src/domain.py",))
    source_path.write_text("value = 2\n", encoding="utf-8")
    second = source_fingerprint(tmp_path, ("src/domain.py",))

    assert first != second


def test_remediation_debt_rejects_growth_and_stale_fingerprint(tmp_path: Path) -> None:
    """Force touched mixed files to update both their cap and current-state record."""

    source_path = _initialize_fixture(
        tmp_path,
        source="\n".join(f"value_{index} = {index}" for index in range(6)) + "\n",
    )
    fingerprint = source_fingerprint(tmp_path, ("src/domain.py",))
    (tmp_path / "ARCHITECTURE_DEBT.toml").write_text(
        f"""schema = 1
[[debt]]
id = "DEBT-001"
owner = "fixture owner"
paths = ["src/domain.py"]
fingerprint = "{fingerprint}"
review_by = 2099-01-01
responsibilities = ["one", "two"]
next_extraction = "Extract responsibility two."
""",
        encoding="utf-8",
    )
    (tmp_path / "ARCHITECTURE_WAIVERS.toml").write_text(
        """schema = 1
[[waiver]]
id = "WAIVER-001"
kind = "remediation"
rule = "STRUCT003"
path = "src/domain.py"
owner = "fixture owner"
justification = "The fixture intentionally models mixed current state."
review_by = 2099-01-01
max_lines = 6
debt = "DEBT-001"
next_limit = 5
""",
        encoding="utf-8",
    )
    assert check_repository(tmp_path).succeeded

    source_path.write_text(
        source_path.read_text(encoding="utf-8") + "value_6 = 6\n",
        encoding="utf-8",
    )
    result = check_repository(tmp_path)

    assert not result.succeeded
    assert {item.rule for item in result.diagnostics} >= {
        "DEBT_STALE",
        "WAIVER_LIMIT",
        "WAIVER_RATCHET",
    }


def test_expired_and_unused_waivers_fail_closed(tmp_path: Path) -> None:
    """Reject stale exception inventory even when source currently passes."""

    _initialize_fixture(tmp_path)
    (tmp_path / "ARCHITECTURE_WAIVERS.toml").write_text(
        """schema = 1
[[waiver]]
id = "WAIVER-001"
kind = "structural"
rule = "STRUCT003"
path = "src/domain.py"
owner = "fixture owner"
justification = "This intentionally stale fixture must be rejected."
review_by = 2020-01-01
max_lines = 5
""",
        encoding="utf-8",
    )

    result = check_repository(tmp_path)

    assert {item.rule for item in result.diagnostics} >= {
        "WAIVER_EXPIRED",
        "WAIVER_UNUSED",
    }


def test_python_dependency_direction_is_enforced(tmp_path: Path) -> None:
    """Reject Python imports from a protected lower layer into an outer layer."""

    _initialize_fixture(tmp_path, source="import outer.adapter\n")
    _write_policy(
        tmp_path,
        dependencies="""
[[dependency]]
name = "DOMAIN"
paths = ["src/*.py"]
forbidden_python_imports = ["outer"]
""",
    )

    result = check_repository(tmp_path)

    assert "DEPENDENCY_DOMAIN" in {item.rule for item in result.diagnostics}


def test_typescript_dependency_direction_is_enforced(tmp_path: Path) -> None:
    """Reject TypeScript imports from a protected lower layer into UI code."""

    source_root = tmp_path / "src"
    source_root.mkdir()
    (source_root / "core.ts").write_text(
        "import { render } from './ui.js';\nexport const value = render;\n",
        encoding="utf-8",
    )
    (source_root / "ui.ts").write_text("export const render = 1;\n", encoding="utf-8")
    _write_policy(
        tmp_path,
        dependencies="""
[[dependency]]
name = "CORE"
paths = ["src/core.ts"]
forbidden_typescript_paths = ["src/ui"]
""",
    )
    _write_empty_registries(tmp_path)

    result = check_repository(tmp_path)

    assert "DEPENDENCY_CORE" in {item.rule for item in result.diagnostics}


def test_system_git_policy_rejects_runtime_processes_and_discovery(
    tmp_path: Path,
) -> None:
    """Prevent runtime code from acquiring or invoking a Git executable."""

    source_root = tmp_path / "sugarcubes"
    source_root.mkdir()
    (source_root / "process.py").write_text(
        "import subprocess\nsubprocess.run(['git.exe', 'status'])\n",
        encoding="utf-8",
    )
    (source_root / "discovery.py").write_text(
        "import shutil\nvalue = shutil.which('git')\n",
        encoding="utf-8",
    )

    diagnostics = scan_system_git_dependencies(tmp_path)

    assert {(item.path, item.rule) for item in diagnostics} == {
        ("sugarcubes/discovery.py", "GIT001"),
        ("sugarcubes/process.py", "GIT001"),
    }


def test_system_git_policy_rejects_gitpython_configuration(tmp_path: Path) -> None:
    """Prevent GitPython from restoring an implicit system-Git dependency."""

    source_root = tmp_path / "sugarcubes"
    source_root.mkdir()
    (source_root / "adapter.py").write_text(
        "import git\nVARIABLE = 'GIT_PYTHON_GIT_EXECUTABLE'\n",
        encoding="utf-8",
    )

    diagnostics = scan_system_git_dependencies(tmp_path)

    assert len(diagnostics) == 1
    assert diagnostics[0].rule == "GIT001"


def test_system_git_policy_allows_internal_python_processes(tmp_path: Path) -> None:
    """Allow bounded subprocess isolation that never invokes system Git."""

    source_root = tmp_path / "sugarcubes"
    source_root.mkdir()
    (source_root / "worker.py").write_text(
        "import subprocess, sys\nsubprocess.run([sys.executable, 'worker.py'])\n",
        encoding="utf-8",
    )

    assert scan_system_git_dependencies(tmp_path) == ()


def test_staged_validation_reads_only_the_git_index(tmp_path: Path) -> None:
    """Validate staged source and governance together, independent of worktree edits."""

    source_path = _initialize_fixture(tmp_path)
    _run_git(tmp_path, "init", "--quiet")
    _run_git(tmp_path, "add", ".")
    source_path.write_text(
        "\n".join(f"value_{index} = {index}" for index in range(6)) + "\n",
        encoding="utf-8",
    )

    assert check_repository(tmp_path, staged=True).succeeded
    assert not check_repository(tmp_path).succeeded

    _run_git(tmp_path, "add", "src/domain.py")
    assert not check_repository(tmp_path, staged=True).succeeded


def test_worktree_inventory_excludes_git_ignored_source(tmp_path: Path) -> None:
    """Keep local generated or ignored files outside architecture ownership."""

    _initialize_fixture(tmp_path)
    (tmp_path / ".gitignore").write_text("src/ignored.py\n", encoding="utf-8")
    (tmp_path / "src" / "ignored.py").write_text("ignored = True\n", encoding="utf-8")
    _run_git(tmp_path, "init", "--quiet")

    policy = load_policy(tmp_path / "ARCHITECTURE_POLICY.toml")

    assert source_paths(tmp_path, policy) == ("src/domain.py",)


def test_remediation_waiver_schema_requires_debt_link(tmp_path: Path) -> None:
    """Reject remediation records that do not identify their exact debt owner."""

    waiver_path = tmp_path / "waivers.toml"
    waiver_path.write_text(
        """schema = 1
[[waiver]]
id = "WAIVER-001"
kind = "remediation"
rule = "STRUCT003"
path = "src/domain.py"
owner = "fixture owner"
justification = "This malformed fixture omits its debt link."
review_by = 2099-01-01
max_lines = 6
next_limit = 5
""",
        encoding="utf-8",
    )

    try:
        load_waivers(waiver_path)
    except RegistryError as error:
        assert "requires debt and next_limit" in str(error)
    else:
        raise AssertionError("Malformed remediation waiver unexpectedly loaded")
