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
"""Prove prestartup root redirection preserves Comfy's literal model folder registry."""

from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import sys

import pytest

from sugarcubes_model_roots import ModelRootStore, apply_configured_model_root

DIRECTORY_LINK_KINDS = (
    ("symlink", "junction") if sys.platform == "win32" else ("symlink",)
)


class ClearTracker:
    """Observe cache invalidation without depending on Comfy internals."""

    def __init__(self) -> None:
        """Initialize an untouched cache."""
        self.cleared = False

    def clear(self) -> None:
        """Record a host invalidation."""
        self.cleared = True


class ComfyRegistry:
    """Mirror Comfy's folder_paths construction for one base and models directory."""

    def __init__(self, base_path: Path, models_dir: str) -> None:
        """Seed default categories exactly as Comfy spells them, without touching disk."""
        self.base_path = str(base_path)
        self.models_dir = models_dir
        self.folder_names_and_paths: dict[
            str, tuple[list[str], set[str] | list[str]]
        ] = {
            "checkpoints": (
                [os.path.join(models_dir, "checkpoints")],
                {".safetensors"},
            ),
            "loras": ([os.path.join(models_dir, "loras")], {".safetensors"}),
            "diffusers": ([os.path.join(models_dir, "diffusers")], []),
        }
        self.filename_list_cache = ClearTracker()
        self.cache_helper = ClearTracker()

    def add_extra_path(self, kind: str, path: str) -> None:
        """Register an extra_model_paths entry with Comfy's normalization and dedupe."""
        paths = self.folder_names_and_paths.setdefault(kind, ([], set()))[0]
        normalized = os.path.normpath(path)
        if normalized not in paths:
            paths.append(normalized)

    def snapshot(self) -> tuple[str, dict[str, tuple[list[str], set[str] | list[str]]]]:
        """Capture every observable registry value."""
        return self.models_dir, copy.deepcopy(self.folder_names_and_paths)


def link_directory(kind: str, link: Path, target: Path) -> None:
    """Create a directory symlink or NTFS junction inside an owned temporary tree."""
    if kind == "symlink":
        link.symlink_to(target, target_is_directory=True)
    elif sys.platform == "win32":
        import _winapi

        _winapi.CreateJunction(str(target), str(link))
    else:
        raise ValueError(f"{kind} directory links are unavailable on this platform.")


def linked_model_library(tmp_path: Path, kind: str) -> tuple[Path, Path]:
    """Build a real model library reached through a chain of two directory links.

    Returns:
        The real library directory and the outermost link Comfy is told to use.
    """
    library = tmp_path / "ImageGen Models"
    (library / "checkpoints").mkdir(parents=True)
    (library / "checkpoints" / "model.safetensors").write_bytes(b"weights")
    inner = tmp_path / "ComfyUI" / "models"
    inner.parent.mkdir()
    link_directory(kind, inner, library)
    outer = tmp_path / "linked models"
    link_directory(kind, outer, inner)
    return library, outer


@pytest.mark.parametrize("kind", DIRECTORY_LINK_KINDS)
def test_default_selection_keeps_comfy_models_directory_through_links(
    tmp_path: Path, kind: str
) -> None:
    """A linked --models-directory and its extra roots survive without a custom selection."""
    library, linked_models = linked_model_library(tmp_path, kind)
    base = tmp_path / "isolated base"
    base.mkdir()
    registry = ComfyRegistry(base, str(linked_models))
    registry.add_extra_path("checkpoints", str(library / "checkpoints"))
    registry.add_extra_path("upscale_models", str(library / "upscale_models"))
    before = registry.snapshot()

    assert apply_configured_model_root(registry) == linked_models

    assert registry.snapshot() == before
    assert not (base / "models").exists()
    assert not registry.filename_list_cache.cleared
    assert not registry.cache_helper.cleared


@pytest.mark.parametrize("kind", DIRECTORY_LINK_KINDS)
def test_custom_selection_redirects_only_comfy_default_entries(
    tmp_path: Path, kind: str
) -> None:
    """Extra roots inside the linked library keep their exact spelling and order."""
    library, linked_models = linked_model_library(tmp_path, kind)
    base = tmp_path / "isolated base"
    shared = tmp_path / "shared"
    ModelRootStore(base).save(shared)
    configured = ModelRootStore(base).load()
    assert configured is not None
    registry = ComfyRegistry(base, str(linked_models))
    registry.add_extra_path("checkpoints", str(library / "checkpoints"))
    nested = os.path.join(str(linked_models), "nested", "loras")
    registry.add_extra_path("loras", nested)
    registry.add_extra_path("ultralytics", str(library / "ultralytics"))

    for _ in range(2):
        assert apply_configured_model_root(registry) == configured

    assert registry.models_dir == str(configured)
    paths = {
        category: entry[0]
        for category, entry in registry.folder_names_and_paths.items()
    }
    assert paths == {
        "checkpoints": [str(configured / "checkpoints"), str(library / "checkpoints")],
        "loras": [str(configured / "loras"), nested],
        "diffusers": [str(configured / "diffusers")],
        "ultralytics": [str(library / "ultralytics")],
    }
    assert registry.folder_names_and_paths["diffusers"][1] == []
    assert registry.filename_list_cache.cleared
    assert registry.cache_helper.cleared
    assert (library / "checkpoints" / "model.safetensors").read_bytes() == b"weights"


@pytest.mark.parametrize("kind", DIRECTORY_LINK_KINDS)
def test_custom_selection_of_the_linked_active_directory_changes_nothing(
    tmp_path: Path, kind: str
) -> None:
    """Selecting the active library through a link never respells registered paths."""
    library, linked_models = linked_model_library(tmp_path, kind)
    base = tmp_path / "isolated base"
    ModelRootStore(base).save(tmp_path / "ComfyUI" / "models")
    registry = ComfyRegistry(base, str(linked_models))
    registry.add_extra_path("checkpoints", str(library / "checkpoints"))
    before = registry.snapshot()

    assert apply_configured_model_root(registry) == linked_models

    assert registry.snapshot() == before
    assert not registry.filename_list_cache.cleared


def test_redirect_collapses_into_an_existing_external_entry(tmp_path: Path) -> None:
    """A redirected default never duplicates an extra root with the same spelling."""
    base = tmp_path / "ComfyUI"
    shared = tmp_path / "shared"
    configured = ModelRootStore(base).save(shared)
    assert configured is not None
    registry = ComfyRegistry(base, str(base / "models"))
    registry.add_extra_path("checkpoints", str(configured / "checkpoints"))

    apply_configured_model_root(registry)

    assert registry.folder_names_and_paths["checkpoints"][0] == [
        str(configured / "checkpoints")
    ]


def test_selection_belongs_to_comfy_base_path(tmp_path: Path) -> None:
    """Only the host installation's selection applies, wherever the extension lives."""
    host = tmp_path / "host"
    other = tmp_path / "extension checkout parent"
    ModelRootStore(other).save(tmp_path / "unrelated")
    registry = ComfyRegistry(host, str(host / "models"))
    before = registry.snapshot()

    apply_configured_model_root(registry)

    assert registry.snapshot() == before
    assert not host.exists()


def test_legacy_migration_retains_original_bytes_and_retires_hook(
    tmp_path: Path,
) -> None:
    """Recoverable migration removes competing startup behavior without destroying user bytes."""
    comfy = tmp_path / "ComfyUI"
    legacy = comfy / ".substitute" / "managed_model_root.json"
    legacy.parent.mkdir(parents=True)
    original = json.dumps({"schema_version": 1, "model_root": str(tmp_path / "shared")})
    legacy.write_text(original, encoding="utf-8")
    hook = comfy / "custom_nodes" / "SubstituteManagedModelRoot"
    hook.mkdir(parents=True)
    (hook / "prestartup_script.py").write_text("historical hook", encoding="utf-8")
    registry = ComfyRegistry(comfy, str(comfy / "models"))
    apply_configured_model_root(registry)
    assert registry.models_dir == str((tmp_path / "shared").resolve())
    assert not legacy.exists()
    assert not hook.exists()
    backup = tuple(legacy.parent.glob("managed_model_root.json.migrated-*"))
    assert len(backup) == 1
    assert backup[0].read_text(encoding="utf-8") == original
    retired = tuple((legacy.parent / "retired-hooks").iterdir())
    assert (retired[0] / "prestartup_script.py").read_text(
        encoding="utf-8"
    ) == "historical hook"


def test_unknown_configuration_leaves_registry_untouched(tmp_path: Path) -> None:
    """A future schema stops startup before any registry change."""
    store = ModelRootStore(tmp_path)
    store.config_path.parent.mkdir(parents=True)
    original = '{"schemaVersion":900,"modelRoot":"future"}'
    store.config_path.write_text(original, encoding="utf-8")
    registry = ComfyRegistry(tmp_path, str(tmp_path / "models"))
    before = registry.snapshot()
    with pytest.raises(ValueError, match="Unsupported"):
        apply_configured_model_root(registry)
    assert store.config_path.read_text(encoding="utf-8") == original
    assert registry.snapshot() == before


def test_malformed_registry_is_rejected_before_any_write(tmp_path: Path) -> None:
    """A host surface without its installation path cannot select a configuration."""
    registry = ComfyRegistry(tmp_path / "host", str(tmp_path / "models"))
    del registry.base_path
    with pytest.raises(ValueError, match="unavailable"):
        apply_configured_model_root(registry)
    assert not (tmp_path / "host").exists()
