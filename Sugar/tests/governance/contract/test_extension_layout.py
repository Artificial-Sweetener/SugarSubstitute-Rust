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
"""SugarCubes installed-layout ownership tests."""

from __future__ import annotations

import importlib.util
import os
import subprocess
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from sugarcubes.extension_layout import extension_root

PACK_ROOT = Path(__file__).resolve().parents[3]


def test_extension_root_resolves_the_comfy_extension_directory() -> None:
    """All execution modes should resolve the same extension-owned data root."""

    assert extension_root() == PACK_ROOT
    assert extension_root() / ".sugarcubes" == PACK_ROOT / ".sugarcubes"


@contextmanager
def _directory_link(link: Path, target: Path) -> Iterator[None]:
    """Link a folder the way an ordinary account can: a junction on Windows."""

    if os.name == "nt":
        subprocess.run(
            ["cmd.exe", "/d", "/c", "mklink", "/J", str(link), str(target)],
            check=True,
            capture_output=True,
            timeout=30,
        )
    else:
        link.symlink_to(target, target_is_directory=True)
    try:
        yield
    finally:
        # Remove only the link itself; the pack it points at must stay untouched.
        if os.name == "nt":
            link.rmdir()
        else:
            link.unlink()


def test_extension_root_keeps_a_linked_install_location(tmp_path: Path) -> None:
    """Own data and sibling custom nodes where ComfyUI loaded a linked development install."""

    custom_nodes = tmp_path / "custom_nodes"
    custom_nodes.mkdir()
    installed = custom_nodes / "SugarCubes"
    with _directory_link(installed, PACK_ROOT):
        spec = importlib.util.spec_from_file_location(
            "linked_extension_layout", installed / "sugarcubes" / "extension_layout.py"
        )
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        linked_root = module.extension_root()
        assert linked_root == installed
        assert linked_root.parent == custom_nodes
        assert linked_root.resolve() == PACK_ROOT
