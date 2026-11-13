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
"""The real SugarCubes archive installs as a registry pack its own updater manages.

Builds the archive from this checkout's ``Sugar/`` exactly as the release does, installs it
the way ComfyUI Manager installs a registry pack (members extracted into
``custom_nodes/SugarCubes`` and listed in ``.tracking``), then asks the *installed* copy's own
code to classify the folder. Needs the pinned comfy-cli and SugarCubes' Python dependencies
(``pip install -e "Sugar[dev]"``): the installed pack is imported, not inspected as text.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import tomllib
import unittest
import zipfile
from pathlib import Path

RELEASE = Path(__file__).resolve().parents[1]
SUGAR = RELEASE.parents[1] / "Sugar"
SCRIPT = RELEASE / "sugarcubes_registry.py"

# Runs inside the installed pack: installs nothing, only reports what its own code concludes.
INSPECT = """
import json, sys
from pathlib import Path
pack = Path(sys.argv[1])
sys.path.insert(0, str(pack))
import sugarcubes
from sugarcubes.backend.composition import build_backend_services
from sugarcubes.backend.pack_install.ownership import OwnedPack, inspect_pack_folder
owned = inspect_pack_folder(pack)
snapshot = build_backend_services(pack).self_update.snapshot()
print(json.dumps({
    "module": str(Path(sugarcubes.__file__).resolve()),
    "owned": isinstance(owned, OwnedPack),
    "name": owned.identity.name if isinstance(owned, OwnedPack) else None,
    "version": str(owned.identity.version) if isinstance(owned, OwnedPack) else None,
    "tracked": len(owned.entries) if isinstance(owned, OwnedPack) else 0,
    "phase": snapshot.phase.value,
    "not_managed": None if snapshot.not_managed is None else snapshot.not_managed.value,
}))
"""

# Installs as ComfyUI Manager does, with the validated extractor SugarCubes' updater uses.
INSTALL = """
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[3])
from sugarcubes.backend.pack_install.archive import extract_registry_archive
from sugarcubes.backend.pack_install.tracking import render_tracking
pack = Path(sys.argv[2])
extracted = extract_registry_archive(Path(sys.argv[1]), pack)
(pack / ".tracking").write_text(render_tracking(extracted.entries), encoding="utf-8")
"""


def _python(*arguments: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run a fresh isolated interpreter, so no module is shared with this process."""
    return subprocess.run(
        [sys.executable, "-I", *arguments],
        cwd=cwd,
        capture_output=True,
        text=True,
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
    )


class RegistryInstallTest(unittest.TestCase):
    archive: Path
    name: str
    version: str
    scratch: tempfile.TemporaryDirectory[str]

    @classmethod
    def setUpClass(cls) -> None:
        cls.scratch = tempfile.TemporaryDirectory()
        pyproject = tomllib.loads(
            (SUGAR / "pyproject.toml").read_text(encoding="utf-8")
        )
        cls.name = pyproject["project"]["name"]
        cls.version = pyproject["project"]["version"]
        cls.archive = Path(cls.scratch.name) / "dist" / f"SugarCubes-{cls.version}.zip"
        built = _python(str(SCRIPT), "pack", "--output", str(cls.archive), cwd=SUGAR)
        if built.returncode != 0:
            raise AssertionError(built.stderr)
        digest = hashlib.sha256(cls.archive.read_bytes()).hexdigest()
        if built.stdout.strip() != f"sha256:{digest}":
            raise AssertionError(f"pack reported {built.stdout!r} for sha256:{digest}")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.scratch.cleanup()

    def test_archive_holds_only_tracked_pack_files_with_the_runtime_at_its_root(
        self,
    ) -> None:
        listed = subprocess.run(
            ["git", "-c", "core.quotePath=false", "ls-files", "-z"],
            cwd=SUGAR,
            capture_output=True,
            check=True,
        ).stdout.decode("utf-8")
        tracked = {name for name in listed.split("\0") if name}
        with zipfile.ZipFile(self.archive) as archive:
            members = {name for name in archive.namelist() if not name.endswith("/")}
        self.assertLessEqual(members, tracked, "the archive holds untracked files")
        runtime = {
            name
            for name in tracked
            if name
            in {"__init__.py", "pyproject.toml", "prestartup_script.py", "LICENSE"}
            or name.startswith(("sugarcubes/", "web/"))
        }
        self.assertTrue({"__init__.py", "pyproject.toml"} <= runtime)
        self.assertEqual(runtime - members, set(), "the archive misses runtime files")

    def test_installed_archive_is_a_registry_pack_its_updater_manages(self) -> None:
        root = Path(self.scratch.name) / "comfy"
        pack = root / "custom_nodes" / "SugarCubes"
        installed = _python(
            "-c", INSTALL, str(self.archive), str(pack), str(SUGAR), cwd=root.parent
        )
        self.assertEqual(installed.returncode, 0, installed.stderr)
        inspected = _python("-c", INSPECT, str(pack), cwd=root.parent)
        self.assertEqual(inspected.returncode, 0, inspected.stderr)
        report = json.loads(inspected.stdout.strip().splitlines()[-1])
        self.assertTrue(
            Path(report["module"]).is_relative_to(pack.resolve()),
            f"imported {report['module']} instead of the installed pack",
        )
        with zipfile.ZipFile(self.archive) as archive:
            entries = len(archive.namelist())
        self.assertEqual(
            {key: report[key] for key in ("owned", "name", "version", "tracked")},
            {
                "owned": True,
                "name": self.name,
                "version": self.version,
                "tracked": entries,
            },
        )
        self.assertEqual((report["phase"], report["not_managed"]), ("idle", None))


if __name__ == "__main__":
    unittest.main()
