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
"""Select and run SugarCubes proofs from behavioral source ownership."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from tools.architecture.checker import check_repository
from tools.architecture.model import Diagnostic
from tools.architecture.snapshot import repository_snapshot
from tools.testing.model import SelectionReason, TestGroup, TestPolicy, TestSelection
from tools.testing.policy import TestPolicyError, load_test_policy
from tools.testing.runner import run_isolated_groups, run_selection
from tools.testing.selection import (
    TestSelectionError,
    changed_paths,
    select_paths,
    validate_inventory,
)

POLICY_PATH = "TEST_POLICY.toml"


def _parser() -> argparse.ArgumentParser:
    """Build the authoritative test-policy command parser."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("list", help="list declared area/proof groups")
    subparsers.add_parser("validate", help="validate source and test inventory")
    subparsers.add_parser(
        "all",
        help="run every proof with policy-declared isolation",
    )
    explain = subparsers.add_parser("explain", help="explain selection for one path")
    explain.add_argument("path")
    run = subparsers.add_parser("run", help="run one area or exact proof group")
    run.add_argument("area")
    run.add_argument("proof", nargs="?")
    subparsers.add_parser(
        "isolated",
        help="run every proof group in a fresh test process",
    )
    subparsers.add_parser("changed", help="run proofs selected by worktree changes")
    staged = subparsers.add_parser(
        "staged", help="run proofs selected by the Git index"
    )
    staged.add_argument("--commit", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Execute one test-policy command and return its process status."""

    args = _parser().parse_args(list(argv) if argv is not None else None)
    root = Path(__file__).resolve().parents[1]
    staged = args.command == "staged"
    try:
        with repository_snapshot(root, staged=staged) as policy_root:
            policy = load_test_policy(policy_root / POLICY_PATH)
            validate_inventory(policy_root, policy)
        architecture = check_repository(root, staged=staged)
        _render_architecture(architecture.diagnostics)
        if not architecture.succeeded:
            return 1
        if args.command == "list":
            for group in policy.groups:
                sys.stdout.write(f"{group.name}\n")
            return 0
        if args.command == "validate":
            sys.stdout.write("SUCCESS: Test policy and inventory are valid.\n")
            return 0
        if args.command == "isolated":
            return run_isolated_groups(root, policy)
        if args.command == "all":
            selection = TestSelection(
                groups=policy.groups,
                reasons=tuple(
                    SelectionReason(
                        path="<all>",
                        group=group,
                        reason="complete policy gate",
                    )
                    for group in policy.groups
                ),
            )
            _render_selection(selection)
            return run_selection(root, policy, selection)
        if args.command == "explain":
            selection = select_paths(policy, (args.path,))
            _render_selection(selection)
            return 0
        if args.command == "run":
            selection = _explicit_selection(policy, args.area, args.proof)
        else:
            paths = changed_paths(root, staged=staged)
            if staged and not paths:
                raise TestSelectionError(
                    "staged gate requires at least one staged path"
                )
            selection = select_paths(
                policy,
                paths,
                commit=bool(getattr(args, "commit", False)),
            )
        _render_selection(selection)
        if not selection.groups:
            sys.stdout.write("No test groups selected.\n")
            return 0
        return run_selection(root, policy, selection)
    except (KeyError, TestPolicyError, TestSelectionError) as error:
        sys.stderr.write(f"test selection failed: {error}\n")
        return 1


def _explicit_selection(
    policy: TestPolicy,
    area_name: str,
    proof: str | None,
) -> TestSelection:
    """Build an explicit area or area/proof selection."""

    area = policy.area(area_name)
    groups = area.groups if proof is None else (TestGroup(area_name, proof),)
    if any(group not in policy.groups for group in groups):
        raise TestSelectionError(f"unknown proof for {area_name}: {proof}")
    return TestSelection(
        groups=groups,
        reasons=tuple(
            SelectionReason(path="<explicit>", group=group, reason="explicit run")
            for group in groups
        ),
    )


def _render_architecture(diagnostics: tuple[Diagnostic, ...]) -> None:
    """Render architecture diagnostics before selecting behavioral proofs."""

    for diagnostic in diagnostics:
        sys.stdout.write(f"{diagnostic.render()}\n")


def _render_selection(selection: TestSelection) -> None:
    """Render selected groups and their exact reasons."""

    for group in selection.groups:
        sys.stdout.write(f"SELECT {group.name}\n")
        for reason in selection.reasons:
            if reason.group == group:
                sys.stdout.write(f"  {reason.path}: {reason.reason}\n")


if __name__ == "__main__":
    raise SystemExit(main())
