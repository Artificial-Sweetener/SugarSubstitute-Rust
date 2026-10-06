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
"""Apply persisted model-root intent before Comfy initializes model-consuming extensions."""

from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Protocol, cast
from uuid import uuid4

from .store import ModelRootStore

_LOGGER = logging.getLogger(__name__)


class StartupFolderRegistry(Protocol):
    """Describe the validated host state mutated during prestartup only."""

    models_dir: str
    folder_names_and_paths: dict[str, tuple[list[str], set[str] | list[str]]]


def apply_configured_model_root(comfy_root: Path, folder_paths: object) -> Path:
    """Redirect default categories while preserving external roots and historical selections."""
    registry = _validated_registry(folder_paths)
    store = ModelRootStore(comfy_root)
    store.migrate_legacy()
    old_root = Path(registry.models_dir).resolve()
    new_root = store.load() or store.default_root
    new_root.mkdir(parents=True, exist_ok=True)
    if old_root != new_root:
        for kind, (paths, extensions) in tuple(registry.folder_names_and_paths.items()):
            rewritten = [
                (
                    str(new_root / Path(path).resolve().relative_to(old_root))
                    if Path(path).resolve().is_relative_to(old_root)
                    else path
                )
                for path in paths
            ]
            registry.folder_names_and_paths[kind] = (rewritten, extensions)
        registry.models_dir = str(new_root)
        for name in ("filename_list_cache", "cache_helper"):
            clear: object = getattr(getattr(folder_paths, name, None), "clear", None)
            if callable(clear):
                clear()
        _LOGGER.info("SugarCubes applied configured model root")
    _retire_legacy_hook(comfy_root)
    return new_root


def _validated_registry(value: object) -> StartupFolderRegistry:
    """Reject malformed dynamic host state before performing any configuration write."""
    paths: object = getattr(value, "folder_names_and_paths", None)
    if not isinstance(getattr(value, "models_dir", None), str) or not isinstance(
        paths, dict
    ):
        raise ValueError("Comfy's model folder registry is unavailable.")
    for kind, entry in paths.items():
        if (
            not isinstance(kind, str)
            or not isinstance(entry, tuple)
            or len(entry) != 2
            or not isinstance(entry[0], list)
            or any(not isinstance(path, str) for path in entry[0])
            or not isinstance(entry[1], (set, list))
            or any(not isinstance(extension, str) for extension in entry[1])
        ):
            raise ValueError("Comfy's model folder registry is invalid.")
    # The public dynamic module has been narrowed to the exact writable host surface above.
    return cast(StartupFolderRegistry, value)


def _retire_legacy_hook(comfy_root: Path) -> None:
    """Move the historical generated hook outside custom_nodes without deleting its contents."""
    hook = comfy_root / "custom_nodes" / "SubstituteManagedModelRoot"
    if not hook.exists():
        return
    if hook.is_symlink() or not hook.is_dir():
        raise ValueError(
            "The legacy model-root hook has an unexpected filesystem type."
        )
    retired = comfy_root / ".substitute" / "retired-hooks"
    retired.mkdir(parents=True, exist_ok=True)
    os.replace(hook, retired / f"SubstituteManagedModelRoot-{uuid4().hex}")
    _LOGGER.info("SugarCubes retired the historical model-root startup hook")
