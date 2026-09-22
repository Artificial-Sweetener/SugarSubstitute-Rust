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
"""Validate test inventory and select required proofs from changed paths."""

from __future__ import annotations

import subprocess
from pathlib import Path, PurePosixPath

from tools.architecture.registry import load_policy
from tools.architecture.scanner import source_paths, visible_repository_paths

from .model import SelectionReason, TestGroup, TestPolicy, TestSelection


class TestSelectionError(ValueError):
    """Report incomplete inventory or unsafe zero-test source selection."""


_RUNTIME_EXTENSIONS = frozenset(
    {".py", ".pyi", ".ts", ".tsx", ".js", ".json", ".toml", ".yml", ".yaml"}
)


def validate_inventory(root: Path, policy: TestPolicy) -> None:
    """Require total source ownership and exact area/proof test placement."""

    architecture = load_policy(root / "ARCHITECTURE_POLICY.toml")
    errors: list[str] = []
    for source_path in source_paths(root, architecture):
        owners = tuple(area.name for area in policy.areas if area.owns(source_path))
        if len(owners) != 1:
            errors.append(
                "source must map to exactly one area: "
                f"{source_path} -> {owners or '<none>'}"
            )
    test_root = root / policy.test_root
    visible = visible_repository_paths(root)
    modules = tuple(
        path
        for path in test_root.rglob("*")
        if path.is_file()
        and _is_test_module(path)
        and (visible is None or path.relative_to(root).as_posix() in visible)
    )
    basenames: dict[str, str] = {}
    declared = frozenset(group.name for group in policy.groups)
    populated: set[str] = set()
    for module_path in modules:
        relative = module_path.relative_to(root).as_posix()
        parts = PurePosixPath(relative).parts
        if len(parts) < 4:
            errors.append(f"test module must live at tests/<area>/<proof>: {relative}")
            continue
        group = f"{parts[1]}/{parts[2]}"
        if group not in declared:
            errors.append(
                f"test module belongs to undeclared group {group}: {relative}"
            )
        else:
            populated.add(group)
        previous = basenames.get(module_path.name)
        if previous is not None:
            errors.append(
                "duplicate test module basename "
                f"{module_path.name}: {previous}, {relative}"
            )
        basenames[module_path.name] = relative
    for group in sorted(declared - populated):
        errors.append(f"declared test group has no test modules: {group}")
    allowed_root_files = {"__init__.py", "conftest.py"}
    for root_path in test_root.iterdir():
        if root_path.is_file() and root_path.name not in allowed_root_files:
            errors.append(f"root test support is not locally owned: {root_path.name}")
    if errors:
        raise TestSelectionError("\n".join(errors))


def select_paths(
    policy: TestPolicy,
    paths: tuple[str, ...],
    *,
    commit: bool = False,
) -> TestSelection:
    """Select deterministic proof groups for changed repository paths."""

    reasons: list[SelectionReason] = []
    selected: set[TestGroup] = set()
    for raw_path in sorted(set(paths)):
        path = raw_path.replace("\\", "/").removeprefix("./")
        if not path:
            continue
        groups = _groups_for_path(policy, path)
        for group, reason in groups:
            selected.add(group)
            reasons.append(SelectionReason(path=path, group=group, reason=reason))
    if commit and selected:
        for group in policy.groups:
            if group not in selected:
                reasons.append(
                    SelectionReason(
                        path="<commit>",
                        group=group,
                        reason="commit mode expands the affected SugarCubes product",
                    )
                )
        selected = set(policy.groups)
    return TestSelection(
        groups=tuple(sorted(selected)),
        reasons=tuple(
            sorted(reasons, key=lambda item: (item.group, item.path, item.reason))
        ),
    )


def _groups_for_path(
    policy: TestPolicy,
    path: str,
) -> tuple[tuple[TestGroup, str], ...]:
    """Return direct and subscribed proof groups for one changed path."""

    test_prefix = f"{policy.test_root}/"
    if path.startswith(test_prefix):
        parts = PurePosixPath(path).parts
        if len(parts) >= 3 and parts[1] == "support":
            return tuple(
                (group, "changed repository-wide test support")
                for group in policy.groups
            )
        if len(parts) >= 4 and _is_test_name(parts[-1]):
            candidate = TestGroup(parts[1], parts[2])
            if candidate not in policy.groups:
                raise TestSelectionError(
                    f"changed test belongs to unknown group: {path}"
                )
            return ((candidate, "changed test module"),)
        if len(parts) >= 3 and parts[2] == "support":
            area = policy.area(parts[1])
            return tuple(
                (group, "changed area-owned test support") for group in area.groups
            )
        if len(parts) == 3 and parts[-1] == "__init__.py":
            area = policy.area(parts[1])
            return tuple(
                (group, "changed area-owned test package") for group in area.groups
            )
        if len(parts) == 2 and parts[-1] in {"__init__.py", "conftest.py"}:
            return tuple(
                (group, "changed repository-wide test configuration")
                for group in policy.groups
            )
        if "__pycache__" in parts:
            return ()
        raise TestSelectionError(
            f"changed test path is outside policy inventory: {path}"
        )
    owned_path = _authored_path(path)
    owners = tuple(area for area in policy.areas if area.owns(owned_path))
    if len(owners) > 1:
        raise TestSelectionError(
            f"runtime change maps to multiple areas: {path} -> {[area.name for area in owners]}"
        )
    if not owners:
        if PurePosixPath(path).suffix.lower() in _RUNTIME_EXTENSIONS:
            raise TestSelectionError(
                f"runtime change is not allowed to select zero tests: {path}"
            )
        return ()
    owner = owners[0]
    ownership_reason = (
        f"generated output derives from {owned_path} owned by {owner.name}"
        if owned_path != path
        else f"source is owned by {owner.name}"
    )
    direct = [(group, ownership_reason) for group in owner.groups]
    boundary_names = frozenset(
        boundary.name for boundary in policy.boundaries if boundary.area == owner.name
    )
    subscribed = [
        (group, f"consumer subscribes to {subscription.boundary}")
        for subscription in policy.subscriptions
        if subscription.boundary in boundary_names
        for group in subscription.groups
    ]
    return tuple(direct + subscribed)


def _authored_path(path: str) -> str:
    """Map compiler-owned browser output back to its authored TypeScript path."""

    if path.startswith("web/") and path.endswith(".js"):
        return f"frontend/{path[len('web/') : -len('.js')]}.ts"
    return path


def changed_paths(root: Path, *, staged: bool) -> tuple[str, ...]:
    """Return normalized staged or complete worktree changes from Git."""

    if staged:
        return _git_paths(root, "diff", "--cached", "--name-only", "-z")
    tracked = _git_paths(root, "diff", "HEAD", "--name-only", "-z")
    untracked = _git_paths(root, "ls-files", "--others", "--exclude-standard", "-z")
    return tuple(sorted(set(tracked) | set(untracked)))


def _git_paths(root: Path, *args: str) -> tuple[str, ...]:
    """Run one NUL-delimited Git path query."""

    completed = subprocess.run(  # noqa: S603
        ["git", *args],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.decode("utf-8", errors="replace").strip()
        raise TestSelectionError(f"git {' '.join(args)} failed: {message}")
    return tuple(
        value.decode("utf-8", errors="strict").replace("\\", "/")
        for value in completed.stdout.split(b"\0")
        if value
    )


def _is_test_module(path: Path) -> bool:
    """Return whether one path is a Python or TypeScript test module."""

    return _is_test_name(path.name)


def _is_test_name(name: str) -> bool:
    """Return whether one filename follows a supported test convention."""

    return (name.startswith("test_") and name.endswith(".py")) or name.endswith(
        (".test.ts", ".spec.ts")
    )


__all__ = [
    "TestSelectionError",
    "changed_paths",
    "select_paths",
    "validate_inventory",
]
