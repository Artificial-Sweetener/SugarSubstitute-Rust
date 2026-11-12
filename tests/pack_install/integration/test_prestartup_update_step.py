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
"""Prove ComfyUI's prestartup run installs a chosen update before the pack loads."""

from __future__ import annotations

import os
import runpy
import sys
import types
from pathlib import Path

import pytest

from sugarcubes.backend.pack_install.startup import (
    StartupOutcomeKind,
    read_outcome,
    updates_root,
)
from tests.pack_install.support.install_vectors import make_directory_link
from tests.pack_install.support.pack_fixture import install_registry_pack, stage_release

_REPOSITORY = Path(__file__).resolve().parents[3]
_PRIVATE_MODULE = "_sugarcubes_startup_pack_install"


def _startup_sources() -> dict[str, str]:
    """Return the real files the prestartup step runs, keyed by pack path."""

    roots = [
        _REPOSITORY / "sugarcubes" / "backend" / "pack_install",
        _REPOSITORY / "sugarcubes_model_roots",
    ]
    files = {
        "prestartup_script.py": (_REPOSITORY / "prestartup_script.py").read_text(
            encoding="utf-8"
        ),
        "load_model_roots.py": (_REPOSITORY / "load_model_roots.py").read_text(
            encoding="utf-8"
        ),
    }
    for root in roots:
        for source in sorted(root.glob("*.py")):
            files[source.relative_to(_REPOSITORY).as_posix()] = source.read_text(
                encoding="utf-8"
            )
    return files


def _release(version: str) -> dict[str, str]:
    return {
        "pyproject.toml": f'[project]\nname = "SugarCubes"\nversion = "{version}"\n',
        "__init__.py": f"VERSION = '{version}'\n",
        **_startup_sources(),
    }


def _fake_folder_paths(comfy_root: Path) -> types.ModuleType:
    models = str(comfy_root / "models")
    module = types.ModuleType("folder_paths")
    attributes: dict[str, object] = {
        "base_path": str(comfy_root),
        "models_dir": models,
        "folder_names_and_paths": {
            "checkpoints": ([os.path.join(models, "checkpoints")], {".safetensors"}),
        },
        "filename_list_cache": {},
        "cache_helper": types.SimpleNamespace(clear=lambda: None),
    }
    for name, value in attributes.items():
        setattr(module, name, value)
    return module


def _run_prestartup(
    script: Path, comfy_root: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setitem(sys.modules, "folder_paths", _fake_folder_paths(comfy_root))
    runpy.run_path(str(script), run_name="SugarCubes_prestartup")


def test_prestartup_installs_the_chosen_update_and_forgets_its_code(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The update lands before import, and none of the old code stays loaded."""

    pack = install_registry_pack(tmp_path, _release("1.0.0"))
    store = stage_release(tmp_path, pack, _release("1.1.0"), "1.1.0")
    release = store.load()
    assert release is not None
    store.schedule(release)
    _run_prestartup(pack / "prestartup_script.py", tmp_path, monkeypatch)
    assert (pack / "__init__.py").read_text(encoding="utf-8") == "VERSION = '1.1.0'\n"
    outcome = read_outcome(updates_root(pack))
    assert outcome is not None and outcome.kind is StartupOutcomeKind.INSTALLED
    assert not [name for name in sys.modules if name.startswith(_PRIVATE_MODULE)]


def test_prestartup_leaves_a_linked_pack_folder_alone(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reached through a junction, the pack is seen as linked and never replaced."""

    real_root = tmp_path / "source"
    real_pack = install_registry_pack(real_root, _release("1.0.0"))
    store = stage_release(real_root, real_pack, _release("1.1.0"), "1.1.0")
    release = store.load()
    assert release is not None
    store.schedule(release)
    linked = tmp_path / "comfy" / "custom_nodes" / "SugarCubes"
    linked.parent.mkdir(parents=True)
    make_directory_link(linked, real_pack)
    _run_prestartup(linked / "prestartup_script.py", tmp_path / "comfy", monkeypatch)
    assert (real_pack / "__init__.py").read_text(
        encoding="utf-8"
    ) == "VERSION = '1.0.0'\n"
    outcome = read_outcome(updates_root(real_pack))
    assert outcome is not None
    assert (outcome.kind, outcome.reason) == (
        StartupOutcomeKind.NOT_INSTALLED,
        "linked-folder",
    )
