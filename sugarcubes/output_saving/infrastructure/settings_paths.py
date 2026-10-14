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
"""Validate server filesystem destinations and render nonmutating output examples."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

from ..domain.path_pattern import OutputNamingContext, render_components
from ..domain.preferences import OutputSavingPreferences


class HostSavingSettingsPaths:
    """Own configured root interpretation independently of clients' local filesystems."""

    def __init__(self, default_root: Path) -> None:
        """Receive Comfy's authoritative output directory from composition."""
        self._default_root = default_root

    def root(self, preferences: OutputSavingPreferences) -> Path:
        """Validate an absolute directory while preserving configured default-root semantics."""
        root = (
            Path(preferences.organization.output_root)
            if preferences.organization.output_root is not None
            else self._default_root
        )
        if not root.is_absolute():
            raise ValueError(
                "Output folder must be an absolute path on the Comfy server."
            )
        if root.exists() and not root.is_dir():
            raise ValueError("Output folder must be a directory on the Comfy server.")
        return root.resolve()

    def validate(self, preferences: OutputSavingPreferences) -> None:
        """Exercise actual path containment without creating folders or encoding images."""
        self.preview(preferences)

    def preview(self, preferences: OutputSavingPreferences) -> str:
        """Match Substitute's deterministic settings example under the effective server root."""
        root = self.root(preferences)
        components = render_components(
            preferences.organization.path_pattern,
            OutputNamingContext(
                workflow="My Workflow",
                source="Main Output",
                cube="Main Output",
                run=7,
                cube_number=1,
                folder_image_number=1,
                started_at=datetime(2026, 5, 1, 14, 32, 9),
                width=1024,
                height=1024,
                seed="123456789",
                node="Save Image",
                node_id="1",
            ),
        )
        path = root.joinpath(*components[:-1], f"{components[-1]}.png").resolve()
        if not path.is_relative_to(root):
            raise ValueError("Output path escapes the configured folder.")
        return str(path)
