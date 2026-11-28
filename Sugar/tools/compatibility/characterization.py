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
"""Locate existing test scenarios alongside the production adapters that they import."""

from __future__ import annotations

import ast
import json
import subprocess
from pathlib import Path


def write_characterizations(
    root: Path, destination: Path, imports: dict[str, list[str]]
) -> None:
    """Give maintainers exact test locations and source descriptions for the indexed assumptions."""
    paths = sorted({path for files in imports.values() for path in files})
    typescript = [path for path in paths if path.endswith(".ts")]
    tests: dict[str, list[dict[str, object]]] = json.loads(
        subprocess.check_output(
            ["node", "--import", "tsx", "scripts/compatibility/characterizations.ts"],
            input=json.dumps(typescript),
            cwd=root,
            encoding="utf-8",
        )
    )
    for path in paths:
        if not path.endswith(".py"):
            continue
        tree = ast.parse((root / path).read_text(encoding="utf-8-sig"))
        tests[path] = [
            {
                "title": node.name,
                "intent": ast.get_docstring(node) or "",
                "line": node.lineno,
            }
            for node in ast.walk(tree)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name.startswith("test_")
        ]
    destination.write_text(
        json.dumps({"imports": imports, "declarations": tests}, indent=2),
        encoding="utf-8",
    )
