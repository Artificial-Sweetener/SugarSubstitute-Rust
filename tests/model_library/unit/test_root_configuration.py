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
"""Safeguard persisted root choices and prestartup redirection across ownership migration."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from sugarcubes_model_roots import (
    ModelRootConfiguration,
    ModelRootStore,
    apply_configured_model_root,
)


class ClearTracker:
    """Observe cache invalidation without depending on Comfy internals."""

    def __init__(self) -> None:
        """Initialize an untouched cache."""
        self.cleared = False

    def clear(self) -> None:
        """Record a host invalidation."""
        self.cleared = True


class RootRegistry:
    """Provide approved default and external categories for startup contract proof."""

    def __init__(self, root: Path, external: Path) -> None:
        """Bind paths without creating or scanning model files."""
        self.models_dir = str(root)
        self.folder_names_and_paths: dict[
            str, tuple[list[str], set[str] | list[str]]
        ] = {
            "checkpoints": ([str(root / "checkpoints")], {".safetensors"}),
            "loras": ([str(root / "loras"), str(external)], {".safetensors"}),
            "diffusers": ([str(root / "diffusers")], []),
        }
        self.filename_list_cache = ClearTracker()
        self.cache_helper = ClearTracker()


def test_configuration_preserves_next_launch_intent_and_unrelated_state(
    tmp_path: Path,
) -> None:
    """Configuration changes require restart and never relocate model data."""
    store = ModelRootStore(tmp_path)
    active = tmp_path / "models"
    configuration = ModelRootConfiguration(store, lambda: active)
    assert not configuration.get_status().restart_required
    selected = tmp_path / "shared models"
    status = configuration.configure("custom", str(selected))
    assert status.restart_required
    assert status.active_model_root == str(active.resolve())
    assert store.load() == selected.resolve()
    assert not tuple(store.config_path.parent.glob("*.tmp"))
    unrelated = store.config_path.parent / "other.json"
    unrelated.write_text("{}", encoding="utf-8")
    configuration.configure("default", None)
    assert not store.config_path.exists()
    assert unrelated.exists()
    with pytest.raises(ValueError, match="absolute"):
        configuration.configure("custom", "relative/models")
    with pytest.raises(ValueError, match="does not accept"):
        configuration.configure("default", str(selected))


def test_prestartup_is_idempotent_and_preserves_extra_paths(tmp_path: Path) -> None:
    """Repeated hooks consume one persisted selection without redirecting external additions."""
    comfy = tmp_path / "ComfyUI"
    shared = tmp_path / "shared"
    extra = tmp_path / "extra"
    ModelRootStore(comfy).save(shared)
    registry = RootRegistry(comfy / "models", extra)
    for _ in range(2):
        assert apply_configured_model_root(comfy, registry) == shared.resolve()
    assert registry.models_dir == str(shared.resolve())
    assert registry.folder_names_and_paths["loras"][0] == [
        str(shared / "loras"),
        str(extra),
    ]
    assert registry.filename_list_cache.cleared
    assert registry.cache_helper.cleared
    assert registry.folder_names_and_paths["diffusers"] == (
        [str(shared / "diffusers")],
        [],
    )


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
    registry = RootRegistry(comfy / "models", tmp_path / "extra")
    apply_configured_model_root(comfy, registry)
    assert not legacy.exists()
    assert not hook.exists()
    backup = tuple(legacy.parent.glob("managed_model_root.json.migrated-*"))
    assert len(backup) == 1
    assert backup[0].read_text(encoding="utf-8") == original
    retired = tuple((legacy.parent / "retired-hooks").iterdir())
    assert (retired[0] / "prestartup_script.py").read_text(
        encoding="utf-8"
    ) == "historical hook"


def test_unknown_configuration_remains_untouched(tmp_path: Path) -> None:
    """A future schema cannot be overwritten by an older startup consumer."""
    store = ModelRootStore(tmp_path)
    store.config_path.parent.mkdir(parents=True)
    original = '{"schemaVersion":900,"modelRoot":"future"}'
    store.config_path.write_text(original, encoding="utf-8")
    registry = RootRegistry(tmp_path / "models", tmp_path / "extra")
    with pytest.raises(ValueError, match="Unsupported"):
        apply_configured_model_root(tmp_path, registry)
    with pytest.raises(ValueError, match="Unsupported"):
        store.save(None)
    with pytest.raises(ValueError, match="Unsupported"):
        store.save(tmp_path / "replacement")
    assert store.config_path.read_text(encoding="utf-8") == original
    assert registry.models_dir == str(tmp_path / "models")
