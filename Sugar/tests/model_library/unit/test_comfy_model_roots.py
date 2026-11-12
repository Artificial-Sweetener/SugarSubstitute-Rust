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
"""Prove configured-root discovery preserves exact host choices and path safety."""

from __future__ import annotations

from pathlib import Path
import shutil

from sugarcubes.model_library.infrastructure.comfy_roots import ComfyModelRoots


from tests.model_library.support.comfy_registry import TestFolderRegistry


def test_active_root_and_extra_root_keep_literal_selection(tmp_path: Path) -> None:
    """Catalog additional configured roots and use the active model folder for storage."""
    registry = TestFolderRegistry(tmp_path)
    model = registry.primary / "Anima/alpha.safetensors"
    model.parent.mkdir(parents=True)
    model.write_bytes(b"fixture model bytes")
    registry.values["Anima\\alpha.safetensors"] = str(model)
    roots = ComfyModelRoots(registry)
    entry = roots.list_files()[0]
    assert roots.storage_root() == registry.models_dir
    assert entry.value == "Anima\\alpha.safetensors"
    assert entry.relative_path == "Anima/alpha.safetensors"
    assert entry.size_bytes == len(b"fixture model bytes")
    registry.folder_names_and_paths["loras"][0].reverse()
    assert roots.list_files()[0].location_id == entry.location_id


def test_explicit_discovery_invalidates_only_requested_host_filename_lists(
    tmp_path: Path,
) -> None:
    """Observe new choices without allowing another consumer to own Comfy cache invalidation."""
    registry = TestFolderRegistry(tmp_path)
    registry.filename_list_cache.update(loras=object(), checkpoints=object())
    ComfyModelRoots(registry).list_files(("loras",))
    assert "loras" not in registry.filename_list_cache
    assert "checkpoints" in registry.filename_list_cache


def test_external_paths_and_traversal_are_not_cataloged(tmp_path: Path) -> None:
    """Do not trust a host adapter to validate user-supplied paths for us."""
    registry = TestFolderRegistry(tmp_path)
    outside = tmp_path / "private.safetensors"
    outside.write_bytes(b"outside approved roots")
    registry.values["escape.safetensors"] = str(outside)
    registry.values["../private.safetensors"] = str(outside)
    roots = ComfyModelRoots(registry)
    assert roots.list_files() == ()
    assert roots.resolve_file("loras", "../private.safetensors") is None
    assert roots.resolve_file("loras", str(outside)) is None


def test_primary_model_identity_survives_moving_the_configured_model_folder(
    tmp_path: Path,
) -> None:
    """Retain portable catalog and user-preview keys when the whole model folder moves."""
    registry = TestFolderRegistry(tmp_path)
    old_root = Path(registry.models_dir)
    model = old_root / "loras/fixture.safetensors"
    model.parent.mkdir(parents=True)
    model.write_bytes(b"fixture")
    registry.folder_names_and_paths["loras"] = ([str(model.parent)], {".safetensors"})
    registry.values["fixture.safetensors"] = str(model)
    before = ComfyModelRoots(registry).list_files()[0]
    moved_root = tmp_path / "relocated-models"
    shutil.copytree(old_root, moved_root)
    registry.models_dir = str(moved_root)
    registry.folder_names_and_paths["loras"] = (
        [str(moved_root / "loras")],
        {".safetensors"},
    )
    registry.values["fixture.safetensors"] = str(
        moved_root / "loras/fixture.safetensors"
    )
    after = ComfyModelRoots(registry).list_files()[0]
    assert after.location_id == before.location_id
