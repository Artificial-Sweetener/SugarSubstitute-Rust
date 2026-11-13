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
"""Keep GIMP's app-specific compatibility override out of the separate WebView2 companion."""

from __future__ import annotations

import subprocess
import sys

from integrations.gimp.native.environment import (
    COMPANION_CREATION_FLAGS,
    companion_environment,
)


def test_companion_does_not_inherit_gimps_high_dpi_compatibility_layer() -> None:
    """Prevent the native WebView2 startup failure observed with GIMP's HighDpiAware environment."""
    source = {
        "__COMPAT_LAYER": "HighDpiAware",
        "SUGARCUBES_GIMP_NATIVE": "0",
        "SUGARCUBES_GIMP_OWNER": "Unrelated inherited window",
        "SUGARCUBES_COMFY_URL": "http://127.0.0.1:32981/",
        "PATH": "GIMP SDK path",
        "TEMP": "D:\\native-proof-temp",
    }
    actual = companion_environment(source)
    assert "__COMPAT_LAYER" not in actual
    assert actual["SUGARCUBES_GIMP_NATIVE"] == "1"
    assert "SUGARCUBES_GIMP_OWNER" not in actual
    assert actual["SUGARCUBES_COMFY_URL"] == source["SUGARCUBES_COMFY_URL"]
    assert actual["PATH"] == source["PATH"]
    assert actual["TEMP"] == source["TEMP"]
    assert source["__COMPAT_LAYER"] == "HighDpiAware"
    assert source["SUGARCUBES_GIMP_NATIVE"] == "0"


def test_companion_children_never_open_a_console_window() -> None:
    """Hide every companion child's console on Windows and pass POSIX no creation flags."""
    if sys.platform == "win32":
        assert COMPANION_CREATION_FLAGS == subprocess.CREATE_NO_WINDOW
    else:
        assert COMPANION_CREATION_FLAGS == 0
