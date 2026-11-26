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
"""Require the spellchecker's dictionary and module to be recorded, licensed and deployed as built."""

from __future__ import annotations

import hashlib
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[3]


def _component(manifest: dict[str, object], name: str) -> dict[str, object]:
    components = manifest["component"]
    assert isinstance(components, list)
    found = [c for c in components if isinstance(c, dict) and c.get("name") == name]
    assert len(found) == 1, name
    component: dict[str, object] = found[0]
    return component


def test_the_dictionary_is_pinned_hashed_and_deployed_unchanged() -> None:
    """Each dictionary file matches its recorded hash, and the deployed copy is the same bytes."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    dictionary = _component(manifest, "SCOWL en_US Hunspell dictionary")
    assert re.fullmatch(r"[0-9a-f]{40}", str(dictionary["revision"]))
    license_file = ROOT / str(dictionary["license_file"])
    assert "SCOWL" in license_file.read_text("utf-8")
    assets = dictionary["asset"]
    assert isinstance(assets, list) and len(assets) == 2
    for asset in assets:
        assert isinstance(asset, dict)
        contents = (ROOT / str(asset["vendored_file"])).read_bytes()
        assert hashlib.sha256(contents).hexdigest() == asset["sha256"]
        assert (ROOT / str(asset["runtime_file"])).read_bytes() == contents


def test_the_module_and_its_crates_are_licensed_and_deployed_as_built() -> None:
    """spellbook and the crates compiled with it name the deployed module and ship their licences."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    module = ROOT / "frontend/static/spellcheck/sugarcubes-spellcheck.wasm"
    assert module.read_bytes().startswith(b"\x00asm")
    assert (
        ROOT / "web/spellcheck/sugarcubes-spellcheck.wasm"
    ).read_bytes() == module.read_bytes()
    lock = (ROOT / "spellcheck/Cargo.lock").read_text("utf-8")
    for name in ("spellbook", "hashbrown", "foldhash"):
        component = _component(manifest, name)
        assert component["runtime_files"] == [
            "web/spellcheck/sugarcubes-spellcheck.wasm"
        ]
        assert (ROOT / str(component["license_file"])).is_file()
        assert f'name = "{name}"\nversion = "{component["version"]}"' in lock
    notice = (ROOT / "third_party/NOTICE.md").read_text("utf-8")
    assert "spellbook" in notice and "SCOWL" in notice
