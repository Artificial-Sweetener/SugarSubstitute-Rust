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
"""Prove generated packages cannot enter authored JavaScript test discovery."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys


def test_jest_ignores_nested_generated_packages_and_their_tests(tmp_path: Path) -> None:
    """Exercise the real crawler with colliding package names outside the test owner."""
    repository = Path(__file__).resolve().parents[3]
    configuration: dict[str, object] = json.loads(
        (repository / "jest.config.json").read_text(encoding="utf-8")
    )
    configuration.update(
        rootDir=str(tmp_path), transform={}, setupFiles=[], moduleNameMapper={}
    )
    authored = tmp_path / "tests" / "example" / "unit" / "authored.test.ts"
    generated = tmp_path / "build" / "package" / "tests" / "generated.test.ts"
    for test in (authored, generated):
        test.parent.mkdir(parents=True)
        test.write_text("export {};\n", encoding="utf-8")
    for directory in (tmp_path, tmp_path / "build" / "package"):
        (directory / "package.json").write_text(
            '{"name":"SugarCubes"}', encoding="utf-8"
        )
    node = shutil.which("node")
    assert node is not None, "Jest discovery requires the repository's Node runtime."
    result = subprocess.run(
        [
            node,
            str(repository / "node_modules" / "jest" / "bin" / "jest.js"),
            "--config",
            json.dumps(configuration),
            "--listTests",
            "--runInBand",
            "--json",
        ],
        cwd=repository,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
        creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0,
    )
    discovered: list[str] = json.loads(result.stdout)
    assert [Path(name).resolve() for name in discovered] == [authored.resolve()]
    assert "naming collision" not in result.stderr
