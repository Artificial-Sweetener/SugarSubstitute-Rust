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
"""Resolve model evidence through Comfy's configured folder registry."""

from __future__ import annotations

from collections.abc import Sequence
import hashlib
import importlib
import logging
import os
from pathlib import Path
from typing import Protocol, cast

from ..domain.models import ModelEvidence

_LOGGER = logging.getLogger(__name__)


class FolderPaths(Protocol):
    """Describe only the host API needed for catalog discovery."""

    models_dir: str
    folder_names_and_paths: dict[str, tuple[list[str], set[str]]]
    filename_list_cache: dict[str, object]

    def get_filename_list(self, folder_name: str) -> list[str]:
        """Return exact selectable widget values for one model kind."""

    def get_full_path(self, folder_name: str, filename: str) -> str | None:
        """Resolve a value using the host's configured-root precedence."""

    def get_folder_paths(self, folder_name: str) -> list[str]:
        """Return every configured root for the selected model kind."""


class ComfyModelRoots:
    """Keep folder discovery and approved-path validation at the host boundary."""

    def __init__(self, folder_paths: FolderPaths) -> None:
        """Bind one running Comfy folder registry without importing host globals."""
        self._folders = folder_paths

    @classmethod
    def from_host(cls) -> ComfyModelRoots:
        """Load Comfy's dynamic module only at the integration boundary."""
        module = importlib.import_module("folder_paths")
        required = ("get_filename_list", "get_full_path", "get_folder_paths")
        if (
            not isinstance(getattr(module, "models_dir", None), str)
            or not isinstance(getattr(module, "folder_names_and_paths", None), dict)
            or not isinstance(getattr(module, "filename_list_cache", None), dict)
            or any(not callable(getattr(module, name, None)) for name in required)
        ):
            raise RuntimeError("Comfy's model folder registry is unavailable.")
        # Comfy supplies a dynamic module; its public surface is validated above.
        return cls(cast(FolderPaths, module))

    def storage_root(self) -> str:
        """Use the active configured model folder, including custom model roots."""
        return str(Path(self._folders.models_dir).resolve())

    def configured_folders(self) -> dict[str, tuple[str, ...]]:
        """Observe registered model folders without exposing non-model host categories."""
        return {
            kind: tuple(
                str(Path(root).resolve())
                for root in self._folders.get_folder_paths(kind)
            )
            for kind in self._folders.folder_names_and_paths
            if kind not in ("custom_nodes", "configs", "classifiers")
        }

    def install_root(self, kind: str) -> str:
        """Resolve the first configured model folder without accepting caller-provided paths."""
        if kind not in self._folders.folder_names_and_paths or kind in (
            "custom_nodes",
            "configs",
            "classifiers",
        ):
            raise ValueError(
                "The requested model category is not approved for installation."
            )
        roots = self._folders.get_folder_paths(kind)
        if not roots:
            raise ValueError("The requested model category has no configured folder.")
        selected = next(
            (root for root in roots if Path(root).name.casefold() == kind.casefold()),
            roots[0],
        )
        return str(Path(selected).resolve())

    def list_files(
        self, kinds: Sequence[str] | None = None
    ) -> tuple[ModelEvidence, ...]:
        """Read file stats for host-visible values without touching model contents."""
        selected = (
            kinds
            if kinds is not None
            else tuple(
                kind
                for kind in self._folders.folder_names_and_paths
                if kind not in ("custom_nodes", "configs", "classifiers")
            )
        )
        for kind in selected:
            self._folders.filename_list_cache.pop(kind, None)
        entries: list[ModelEvidence] = []
        for kind in selected:
            if kind not in self._folders.folder_names_and_paths:
                continue
            for value in self._folders.get_filename_list(kind):
                path = self.resolve_file(kind, value)
                if path is None:
                    continue
                model_path = Path(path)
                root = self._approved_root(kind, model_path)
                if root is None:
                    continue
                try:
                    stat = model_path.stat()
                except OSError:
                    _LOGGER.warning(
                        "Model file changed during catalog scan",
                        extra={"kind": kind, "value": value},
                        exc_info=True,
                    )
                    continue
                root_id = self._root_identity(root)
                entries.append(
                    ModelEvidence(
                        kind=kind,
                        value=value,
                        root_id=root_id,
                        relative_path=model_path.relative_to(root).as_posix(),
                        size_bytes=stat.st_size,
                        modified_ns=stat.st_mtime_ns,
                    )
                )
        return tuple(entries)

    def _root_identity(self, root: Path) -> str:
        """Keep roots inside the portable model folder stable across machine and directory moves."""
        model_root = Path(self.storage_root())
        key = (
            "model-root\0" + root.relative_to(model_root).as_posix()
            if root.is_relative_to(model_root)
            else "external-root\0" + str(root)
        )
        return hashlib.sha256(os.path.normcase(key).encode("utf-8")).hexdigest()

    def resolve_file(self, kind: str, value: str) -> str | None:
        """Reject traversal and symlink escapes even when a host resolver accepts them."""
        relative = Path(value.replace("\\", "/"))
        if (
            kind not in self._folders.folder_names_and_paths
            or relative.is_absolute()
            or ".." in relative.parts
            or "\0" in value
        ):
            return None
        resolved = self._folders.get_full_path(kind, value)
        if resolved is None:
            return None
        path = Path(resolved).resolve()
        if not path.is_file() or self._approved_root(kind, path) is None:
            return None
        return str(path)

    def _approved_root(self, kind: str, path: Path) -> Path | None:
        """Return the actual owning root rather than trusting choice-name prefixes."""
        for value in self._folders.get_folder_paths(kind):
            root = Path(value).resolve()
            if path.is_relative_to(root):
                return root
        return None
