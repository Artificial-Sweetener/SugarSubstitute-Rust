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
"""Run dependency-aware local quality gates against worktree or exact Git-index source."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

from tools.architecture.checker import check_repository
from tools.architecture.snapshot import SnapshotError
from tools.testing.policy import load_test_policy
from tools.testing.runner import run_selection
from tools.testing.selection import changed_paths, select_paths, validate_inventory

from .plan import load_checks, select_checks
from .snapshot import quality_snapshot


def main(argv: Sequence[str] | None = None, *, project_root: Path | None = None) -> int:
    """Print selected checks and reasons, then validate the exact selected source tree."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("changed", "staged"))
    parser.add_argument(
        "--plan", action="store_true", help="explain selection without executing checks"
    )
    parser.add_argument(
        "--tests-only",
        action="store_true",
        help="run only the selected behavioral proofs",
    )
    args = parser.parse_args(argv)
    root = project_root or Path(__file__).resolve().parents[2]
    staged = args.mode == "staged"
    try:
        paths = changed_paths(root, staged=staged)
        if staged and not paths:
            raise ValueError("Staged validation requires at least one staged path.")
        with quality_snapshot(root, staged=staged) as snapshot:
            policy = load_test_policy(snapshot / "TEST_POLICY.toml")
            validate_inventory(snapshot, policy)
            removed = frozenset(
                path for path in paths if not (snapshot / path).exists()
            )
            selection = select_paths(policy, paths, removed=removed)
            checks = select_checks(load_checks(snapshot, policy), selection)
            for check in checks:
                sys.stdout.write(f"CHECK {check.name}: npm run {check.script}\n")
            for reason in selection.reasons:
                sys.stdout.write(
                    f"SELECT {reason.group.name}: {reason.path}: {reason.reason}\n"
                )
            if args.plan:
                return 0
            architecture = check_repository(root, staged=staged)
            for diagnostic in architecture.diagnostics:
                sys.stdout.write(f"{diagnostic.render()}\n")
            if not architecture.succeeded:
                return 1
            environment = os.environ.copy()
            if staged:
                for key in (
                    "GIT_INDEX_FILE",
                    "GIT_DIR",
                    "GIT_WORK_TREE",
                    "GIT_COMMON_DIR",
                ):
                    environment.pop(key, None)
            if not args.tests_only:
                npm = shutil.which("npm.cmd" if sys.platform == "win32" else "npm")
                if npm is None:
                    raise ValueError("npm is unavailable.")
                for check in checks:
                    sys.stdout.flush()
                    result = subprocess.run(
                        [npm, "run", check.script],
                        cwd=snapshot,
                        check=False,
                        env=environment,
                    )
                    if result.returncode:
                        return result.returncode
            return run_selection(snapshot, policy, selection, environment=environment)
    except (
        OSError,
        ValueError,
        KeyError,
        SnapshotError,
        subprocess.SubprocessError,
    ) as error:
        sys.stderr.write(f"Quality selection failed: {error}\n")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
