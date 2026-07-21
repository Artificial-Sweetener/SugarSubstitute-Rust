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

from pathlib import Path

from sugarcubes.extension_layout import extension_root


def test_extension_root_resolves_the_comfy_extension_directory() -> None:
    """All execution modes should resolve the same extension-owned data root."""

    expected_root = Path(__file__).resolve().parents[1]

    assert extension_root() == expected_root
    assert extension_root() / ".sugarcubes" == expected_root / ".sugarcubes"
