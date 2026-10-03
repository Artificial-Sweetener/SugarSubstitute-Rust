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
"""Require pinned, licensed, hash-verified icon sources and deployment notices."""

from __future__ import annotations

import hashlib
import re
import tomllib
from pathlib import Path
from xml.etree import ElementTree

ROOT = Path(__file__).resolve().parents[3]


def test_every_vendored_icon_has_exact_provenance_and_deployment_bytes() -> None:
    """Reject unrecorded assets, stale derivatives, unsafe SVGs, and missing notices."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest.get("component")
    assert isinstance(components, list) and components
    recorded: set[str] = set()
    for component in components:
        assert isinstance(component, dict)
        assert (
            component["source"] == "https://github.com/microsoft/fluentui-system-icons"
        )
        assert component["license"] == "MIT"
        assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
        assert component["modifications"] == "None; SVG files copied verbatim."
        license_path = Path(component["license_file"])
        license_bytes = (ROOT / license_path).read_bytes()
        assert b"Copyright (c) 2020 Microsoft Corporation" in license_bytes
        assert b"permission notice shall be included" in license_bytes
        assert (ROOT / "web" / license_path).read_bytes() == license_bytes
        assets: object = component["asset"]
        assert isinstance(assets, list) and assets
        for asset in assets:
            assert isinstance(asset, dict)
            source_path = asset["source_path"]
            authored_path = asset["vendored_file"]
            runtime_path = asset["runtime_file"]
            assert isinstance(source_path, str) and source_path.startswith("assets/")
            assert source_path.endswith("_20_regular.svg")
            assert isinstance(authored_path, str)
            assert authored_path.startswith("frontend/static/icons/")
            assert authored_path not in recorded
            recorded.add(authored_path)
            assert runtime_path == authored_path.replace("frontend/static/", "web/", 1)
            contents = (ROOT / authored_path).read_bytes()
            assert hashlib.sha256(contents).hexdigest() == asset["sha256"]
            assert (ROOT / runtime_path).read_bytes() == contents
            svg = ElementTree.fromstring(contents)
            assert svg.tag == "{http://www.w3.org/2000/svg}svg"
            for element in svg.iter():
                assert element.tag in {svg.tag, "{http://www.w3.org/2000/svg}path"}
                assert not any(key.lower().startswith("on") for key in element.attrib)
                assert not any("href" in key.lower() for key in element.attrib)
    actual = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "frontend/static/icons").rglob("*")
        if path.is_file()
    }
    assert recorded == actual


def test_browser_distribution_preserves_manifest_notice_and_upstream_notice() -> None:
    """Keep licensing available alongside the assets consumed by Comfy's browser."""
    for relative in (
        "third_party/manifest.toml",
        "third_party/NOTICE.md",
        "third_party/licenses/fluentui-system-icons.NOTICE.txt",
    ):
        assert (ROOT / "web" / relative).read_bytes() == (ROOT / relative).read_bytes()
    notice = (ROOT / "third_party/NOTICE.md").read_text("utf-8")
    assert "Microsoft Fluent UI System Icons" in notice
    assert "MIT" in notice
    assert "Copyright (c) 2020 Microsoft Corporation" in notice
