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
"""Provide a typed Comfy folder registry for approved model-root contracts."""

from __future__ import annotations

from pathlib import Path


class TestFolderRegistry:
    """Emulate Comfy's registry boundary without importing the running server."""

    __test__ = False

    def __init__(self, root: Path) -> None:
        """Provide multiple roots whose order can change independently of identity."""
        self.models_dir = str(root / "custom-models")
        self.primary = root / "extra-loras"
        self.secondary = root / "more-loras"
        self.folder_names_and_paths: dict[str, tuple[list[str], set[str]]] = {
            "loras": ([str(self.primary), str(self.secondary)], {".safetensors"})
        }
        self.values: dict[str, str] = {}
        self.filename_list_cache: dict[str, object] = {}

    def get_filename_list(self, folder_name: str) -> list[str]:
        """Return exact dropdown choices from the supplied boundary fixture."""
        return list(self.values) if folder_name == "loras" else []

    def get_full_path(self, folder_name: str, filename: str) -> str | None:
        """Simulate an external resolver whose results still need validation."""
        return self.values.get(filename) if folder_name == "loras" else None

    def get_folder_paths(self, folder_name: str) -> list[str]:
        """Expose configured roots in their current precedence order."""
        return self.folder_names_and_paths[folder_name][0]
