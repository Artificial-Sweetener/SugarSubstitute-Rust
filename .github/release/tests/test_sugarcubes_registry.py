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
"""The registry archive is comfy-cli's own layout: tracked files plus ``[tool.comfy] includes``.

Run with the pinned comfy-cli installed: ``python -m unittest discover -s tests -p "test_*.py"``.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "sugarcubes_registry.py"

PYPROJECT = """\
[project]
name = "sugarcubes"
version = "1.2.3"
description = "fixture"
license = {file = "LICENSE"}

[project.urls]
Repository = "https://github.com/example/sugarcubes"

[tool.comfy]
PublisherId = "example"
DisplayName = "SugarCubes"
Icon = ""
includes = ["web/generated"]
"""


def git(folder: Path, *arguments: str) -> None:
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "Release Test",
        "GIT_AUTHOR_EMAIL": "release-test@example.invalid",
        "GIT_COMMITTER_NAME": "Release Test",
        "GIT_COMMITTER_EMAIL": "release-test@example.invalid",
    }
    subprocess.run(
        ["git", *arguments],
        cwd=folder,
        check=True,
        capture_output=True,
        env=environment,
    )


def tool(
    folder: Path, *arguments: str, token: str = ""
) -> subprocess.CompletedProcess[str]:
    """One command per process, from the pack folder, as the release workflow runs it."""
    environment = {
        key: value
        for key, value in os.environ.items()
        if key != "REGISTRY_ACCESS_TOKEN"
    }
    if token:
        environment["REGISTRY_ACCESS_TOKEN"] = token
    return subprocess.run(
        [sys.executable, str(SCRIPT), *arguments],
        cwd=folder,
        capture_output=True,
        text=True,
        env=environment,
    )


class RegistryArchiveTest(unittest.TestCase):
    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        root = Path(self.scratch.name)
        self.pack = root / "monorepo" / "Sugar"
        (self.pack / "sugarcubes").mkdir(parents=True)
        (self.pack / "web" / "generated").mkdir(parents=True)
        (self.pack / "pyproject.toml").write_text(PYPROJECT, encoding="utf-8")
        (self.pack / "LICENSE").write_text("license\n", encoding="utf-8")
        (self.pack / "__init__.py").write_text(
            "__version__ = '1.2.3'\n", encoding="utf-8"
        )
        (self.pack / "sugarcubes" / "nodes.py").write_text(
            "NODES = {}\n", encoding="utf-8"
        )
        (self.pack / ".gitignore").write_text(
            "web/generated/\nscratch.txt\n", encoding="utf-8"
        )
        (self.pack / "web" / "generated" / "bundle.js").write_text(
            "built\n", encoding="utf-8"
        )
        (self.pack / "scratch.txt").write_text("untracked\n", encoding="utf-8")
        (root / "monorepo" / "README.md").write_text(
            "outside the pack\n", encoding="utf-8"
        )
        git(root / "monorepo", "init", "--quiet")
        git(root / "monorepo", "add", "--all")
        git(root / "monorepo", "commit", "--quiet", "-m", "fixture")

    def tearDown(self) -> None:
        self.scratch.cleanup()

    def test_pack_holds_tracked_pack_files_and_includes_at_the_archive_root(
        self,
    ) -> None:
        output = Path(self.scratch.name) / "dist" / "SugarCubes-1.2.3.zip"
        result = tool(self.pack, "pack", "--output", str(output))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout, r"sha256:[0-9a-f]{64}")
        with zipfile.ZipFile(output) as archive:
            names = sorted(archive.namelist())
        self.assertEqual(
            names,
            [
                ".gitignore",
                "LICENSE",
                "__init__.py",
                "pyproject.toml",
                "sugarcubes/nodes.py",
                "web/generated/bundle.js",
            ],
        )

    def test_pack_refuses_to_overwrite_an_archive(self) -> None:
        output = Path(self.scratch.name) / "SugarCubes-1.2.3.zip"
        output.write_bytes(b"earlier")
        result = tool(self.pack, "pack", "--output", str(output))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists", result.stderr)
        self.assertEqual(output.read_bytes(), b"earlier")

    def test_publish_refuses_an_archive_of_another_version_before_contacting_the_registry(
        self,
    ) -> None:
        notes = Path(self.scratch.name) / "notes.md"
        notes.write_text("notes\n", encoding="utf-8")
        archive = Path(self.scratch.name) / "SugarCubes-9.9.9.zip"
        archive.write_bytes(b"zip")
        arguments = (
            "publish",
            "--archive",
            str(archive),
            "--changelog-file",
            str(notes),
        )
        result = tool(self.pack, *arguments, token="unused")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not the archive of version 1.2.3", result.stderr)
        result = tool(self.pack, *arguments)
        self.assertIn("REGISTRY_ACCESS_TOKEN is not set", result.stderr)


if __name__ == "__main__":
    unittest.main()
