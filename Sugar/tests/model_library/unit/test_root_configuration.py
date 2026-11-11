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
"""Safeguard persisted next-launch model-root choices and their status projection."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes_model_roots import ModelRootConfiguration, ModelRootStore


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


def test_unknown_configuration_cannot_be_overwritten(tmp_path: Path) -> None:
    """A future schema cannot be replaced or removed by an older consumer."""
    store = ModelRootStore(tmp_path)
    store.config_path.parent.mkdir(parents=True)
    original = '{"schemaVersion":900,"modelRoot":"future"}'
    store.config_path.write_text(original, encoding="utf-8")
    with pytest.raises(ValueError, match="Unsupported"):
        store.save(None)
    with pytest.raises(ValueError, match="Unsupported"):
        store.save(tmp_path / "replacement")
    assert store.config_path.read_text(encoding="utf-8") == original
