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
"""Expose SugarCubes' stable model-root configuration API to Comfy host integrations."""

from __future__ import annotations

from collections.abc import Callable
import importlib
from pathlib import Path

from .service import ModelRootConfigurationService
from .domain import ModelRootConfigurationStatus
from .store import (
    ModelRootStore,
    ModelRootSelectionError,
    ModelRootSelectionFailure,
    ModelRootSelectionSource,
)
from .startup import apply_configured_model_root

MODEL_ROOT_API_VERSION = 1


class _RootConfigurationAdapter:
    """Translate filesystem values into normalized application strings."""

    def __init__(self, store: ModelRootStore, active_root: Callable[[], Path]) -> None:
        """Bind a persisted installation selection and its running host state."""
        self._store = store
        self._active_root = active_root

    def default_root(self) -> str:
        """Return the installation's default directory."""
        return str(self._store.default_root)

    def active_root(self) -> str:
        """Normalize running host state at the adapter boundary."""
        return str(self._active_root().resolve())

    def load(self) -> str | None:
        """Narrow stored path values before application policy sees them."""
        value = self._store.load()
        return str(value) if value is not None else None

    def save(self, path: str | None) -> None:
        """Delegate all validation and persistence to the authoritative store."""
        self._store.save(Path(path) if path is not None else None)


class ModelRootConfiguration:
    """Provide one versioned host-facing facade for root selection and status."""

    def __init__(self, store: ModelRootStore, active_root: Callable[[], Path]) -> None:
        """Compose root policy without importing Comfy or altering its current registry."""
        self._service = ModelRootConfigurationService(
            _RootConfigurationAdapter(store, active_root)
        )

    @classmethod
    def from_host(cls) -> ModelRootConfiguration:
        """Validate Comfy's installation path and active model root at the public host boundary."""
        module = importlib.import_module("folder_paths")
        base: object = getattr(module, "base_path", None)
        if not isinstance(base, str) or not isinstance(
            getattr(module, "models_dir", None), str
        ):
            raise RuntimeError(
                "Comfy's model-root configuration surface is unavailable."
            )

        def active_root() -> Path:
            """Narrow active runtime state each time status is requested."""
            value: object = getattr(module, "models_dir", None)
            if not isinstance(value, str):
                raise RuntimeError("Comfy's active model root is unavailable.")
            return Path(value)

        return cls(ModelRootStore(Path(base)), active_root)

    def get_status(self) -> ModelRootConfigurationStatus:
        """Return shared next-launch and active-root state."""
        return self._service.get_status()

    def configure(self, mode: str, path: str | None) -> ModelRootConfigurationStatus:
        """Persist next-launch intent through the sole root-selection owner."""
        return self._service.configure(mode, path)


__all__ = [
    "ModelRootConfiguration",
    "ModelRootStore",
    "ModelRootSelectionError",
    "ModelRootSelectionFailure",
    "ModelRootSelectionSource",
    "apply_configured_model_root",
]
