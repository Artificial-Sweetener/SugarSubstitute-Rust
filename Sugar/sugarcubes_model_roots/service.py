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
"""Own root-selection transitions independently of host registration and storage mechanics."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from .domain import ModelRootConfigurationStatus


class ModelRootConfigurationPort(Protocol):
    """Expose normalized persisted selections and active host state."""

    def default_root(self) -> str:
        """Return the installation's normalized default root."""

    def active_root(self) -> str:
        """Return the running host's normalized root."""

    def load(self) -> str | None:
        """Read persisted next-launch intent."""

    def save(self, path: str | None) -> None:
        """Validate and atomically persist next-launch intent."""


@dataclass(frozen=True, slots=True)
class ModelRootConfigurationService:
    """Give browser and desktop consumers one owner for configuration rules."""

    configuration: ModelRootConfigurationPort

    def get_status(self) -> ModelRootConfigurationStatus:
        """Project persisted intent without mutating the running folder registry."""
        return ModelRootConfigurationStatus(
            self.configuration.default_root(),
            self.configuration.load(),
            self.configuration.active_root(),
        )

    def configure(self, mode: str, path: str | None) -> ModelRootConfigurationStatus:
        """Persist a deliberate default or custom selection for the next launch."""
        if mode == "default":
            if path is not None:
                raise ValueError("Default model-root mode does not accept a path.")
            self.configuration.save(None)
        elif mode == "custom":
            if path is None or not path.strip():
                raise ValueError("Custom model-root mode requires a path.")
            self.configuration.save(path)
        else:
            raise ValueError("Model-root mode must be default or custom.")
        return self.get_status()
