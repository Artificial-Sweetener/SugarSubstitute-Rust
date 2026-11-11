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
"""Check worktree or staged SugarCubes architecture governance."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from tools.architecture.checker import check_repository
from tools.architecture.scanner import source_fingerprint
from tools.architecture.snapshot import repository_snapshot


def main(argv: Sequence[str] | None = None) -> int:
    """Parse architecture commands and return a process exit code."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", action="store_true", help="validate the Git index")
    parser.add_argument(
        "--fingerprint",
        nargs="+",
        metavar="PATH",
        help="print the current-state fingerprint for exact paths",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)
    project_root = Path(__file__).resolve().parents[1]
    if args.fingerprint:
        paths = tuple(str(path).replace("\\", "/") for path in args.fingerprint)
        with repository_snapshot(project_root, staged=args.staged) as root:
            sys.stdout.write(f"{source_fingerprint(root, paths)}\n")
        return 0
    result = check_repository(project_root, staged=args.staged)
    for diagnostic in result.diagnostics:
        sys.stdout.write(f"{diagnostic.render()}\n")
    if not result.succeeded:
        return 1
    warnings = len(result.diagnostics)
    sys.stdout.write(
        f"SUCCESS: Repository architecture is valid ({warnings} structural warnings).\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
