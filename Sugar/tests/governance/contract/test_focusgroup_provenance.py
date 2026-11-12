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
"""Require pinned licensing for the official Fluent keyboard-navigation dependency."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def test_focusgroup_polyfill_has_pinned_licensed_deployment() -> None:
    """Keep the bundled navigation dependency synchronized with its provenance and license."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    matches = [
        item
        for item in components
        if isinstance(item, dict)
        and item.get("name") == "Microsoft focusgroup polyfill"
    ]
    assert len(matches) == 1
    component = matches[0]
    assert component["license"] == "MIT"
    assert component["source"] == "https://github.com/microsoft/polyfills"
    assert component["revision"] == "88cb793b6b8594084e642936d226f2a2e0886982"
    assert component["version"] == "1.6.0"
    lock = json.loads((ROOT / "package-lock.json").read_text("utf-8"))
    assert (
        lock["packages"]["node_modules/@microsoft/focusgroup-polyfill"]["version"]
        == component["version"]
    )
    assert component["source_paths"] == ["packages/focusgroup/src"]
    license_path = Path(component["license_file"])
    license_bytes = (ROOT / license_path).read_bytes()
    assert b"Microsoft Corporation" in license_bytes
    assert b"MIT License" in license_bytes
    assert (ROOT / "web" / license_path).read_bytes() == license_bytes
    assert component["runtime_files"] == [
        "web/comfyui/ui/surface/appearance/FluentThemeRuntime.js"
    ]
    for runtime_path in component["runtime_files"]:
        assert (ROOT / runtime_path).is_file()
    assert "@microsoft/focusgroup-polyfill" in (
        ROOT / "third_party/NOTICE.md"
    ).read_text("utf-8")
