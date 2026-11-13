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
"""Describe configured and active model roots without binding the domain to a filesystem."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class ModelRootConfigurationStatus:
    """Keep next-launch intent separate from the root used by the current process."""

    default_model_root: str
    configured_model_root: str | None
    active_model_root: str

    @property
    def restart_required(self) -> bool:
        """Require a host restart when persisted intent differs from active state."""
        return (
            self.configured_model_root or self.default_model_root
        ) != self.active_model_root

    def to_payload(self) -> dict[str, object]:
        """Preserve the versioned root-status contract used by Substitute clients."""
        return {
            "schemaVersion": 1,
            "defaultModelRoot": self.default_model_root,
            "configuredModelRoot": self.configured_model_root,
            "activeModelRoot": self.active_model_root,
            "usesDefault": self.configured_model_root is None,
            "restartRequired": self.restart_required,
        }
