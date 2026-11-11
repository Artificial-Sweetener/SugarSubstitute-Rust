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

    base_path: str
    models_dir: str
    folder_names_and_paths: dict[str, tuple[list[str], set[str] | list[str]]]


def apply_configured_model_root(folder_paths: object) -> Path:
    """Redirect Comfy's default model categories to a persisted custom root.

    The installation is Comfy's ``base_path``, the same owner the configuration API
    reads, so ``--base-directory`` launches and linked extension checkouts consult
    one selection. Without a custom selection Comfy's own ``models_dir`` stands,
    including ``--models-directory``. Comfy registers model folders as literal
    strings and never resolves junctions or symlinks, so only the default entries
    it spelled as ``os.path.join(models_dir, category)`` move; ``extra_model_paths``
    roots and every other spelling are preserved exactly, and a custom root that is
    the active directory reached through a link leaves the registry untouched.

    Returns:
        The model root Comfy uses for the rest of this process.
    """
    registry = _validated_registry(folder_paths)
    comfy_root = Path(registry.base_path)
    store = ModelRootStore(comfy_root)
    store.migrate_legacy()
    configured_root = store.load()
    active_root = registry.models_dir
    if configured_root is None or _same_directory(active_root, configured_root):
        _retire_legacy_hook(comfy_root)
        return Path(active_root)
    configured_root.mkdir(parents=True, exist_ok=True)
    for kind, (paths, extensions) in tuple(registry.folder_names_and_paths.items()):
        registry.folder_names_and_paths[kind] = (
            _redirect_default_paths(paths, active_root, configured_root),
            extensions,
        )
    registry.models_dir = str(configured_root)
    for name in ("filename_list_cache", "cache_helper"):
        clear: object = getattr(getattr(folder_paths, name, None), "clear", None)
        if callable(clear):
            clear()
    _LOGGER.info("SugarCubes applied configured model root")
    _retire_legacy_hook(comfy_root)
    return configured_root


def _redirect_default_paths(
    paths: list[str], active_root: str, configured_root: Path
) -> list[str]:
    """Move Comfy-derived default entries while keeping the registry duplicate-free.

    Comfy's ``add_model_folder_path`` stores each spelling once; a redirected default
    that equals an existing external entry collapses into that entry's position.
    """
    redirected: list[str] = []
    for path in paths:
        parent, category = os.path.split(path)
        candidate = (
            str(configured_root / category)
            if parent == active_root and category
            else path
        )
        if candidate not in redirected:
            redirected.append(candidate)
    return redirected


def _same_directory(active_root: str, configured_root: Path) -> bool:
    """Recognize one directory reached through different junction or symlink spellings."""
    if os.path.isdir(active_root) and configured_root.is_dir():
        return os.path.samefile(active_root, configured_root)
    return os.path.normcase(os.path.abspath(active_root)) == os.path.normcase(
        str(configured_root)
    )


def _validated_registry(value: object) -> StartupFolderRegistry:
    """Reject malformed dynamic host state before performing any configuration write."""
    paths: object = getattr(value, "folder_names_and_paths", None)
    if (
        not isinstance(getattr(value, "base_path", None), str)
        or not isinstance(getattr(value, "models_dir", None), str)
        or not isinstance(paths, dict)
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
