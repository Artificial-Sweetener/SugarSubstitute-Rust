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
"""Require licensed, pinned deployment of the Spectrum provider and bundled dependencies."""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
RUNTIME = "web/comfyui/ui/surface/appearance/SpectrumThemeRuntime.js"


def test_spectrum_runtime_has_pinned_licensed_deployment() -> None:
    """Verify provider provenance and lockfile integrity for every recorded runtime dependency."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    lock = json.loads((ROOT / "package-lock.json").read_text("utf-8"))
    package = json.loads((ROOT / "package.json").read_text("utf-8"))
    components = [
        component
        for component in manifest["component"]
        if RUNTIME in component.get("runtime_files", [])
    ]
    assert len(components) == 14
    assert (ROOT / RUNTIME).stat().st_size > 100_000
    for component in components:
        license_path = Path(component["license_file"])
        license_bytes = (ROOT / license_path).read_bytes()
        assert len(license_bytes) > 500
        assert license_bytes == (ROOT / "web" / license_path).read_bytes()
        assert component["source_paths"]
        revision = component["revision"]
        if component["source"] == "https://github.com/adobe/spectrum-web-components":
            assert re.fullmatch(r"[0-9a-f]{40}", revision)
            dependency = "@spectrum-web-components/theme"
            assert package["dependencies"][dependency] == component["version"]
        else:
            assert revision.startswith("npm:")
            dependency, version = revision.removeprefix("npm:").rsplit("@", 1)
            assert version == component["version"]
        installed = lock["packages"][f"node_modules/{dependency}"]
        assert installed["version"] == component["version"]
        assert installed["integrity"].startswith("sha512-")
