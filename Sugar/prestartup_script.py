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
"""Prepare SugarCubes before ComfyUI imports any custom node.

First, install a SugarCubes update the user chose, while none of SugarCubes is
loaded; then apply SugarCubes-owned model-root configuration.
"""

from __future__ import annotations

import importlib
import importlib.util
import os
import sys
from pathlib import Path

# The unresolved path keeps a linked pack folder visible to the updater.
_installed_folder = Path(os.path.abspath(__file__)).parent
_extension = Path(__file__).resolve().parent
_PACK_INSTALL_MODULE = "_sugarcubes_startup_pack_install"


def _apply_pending_pack_update() -> None:
    """Run the install-format owner under a private name, then forget it.

    Loading it under a private name, and removing it afterwards, keeps the
    version that ran here out of `sys.modules`, so ComfyUI then imports the
    files that are now on disk.
    """

    package = _extension / "sugarcubes" / "backend" / "pack_install"
    spec = importlib.util.spec_from_file_location(
        _PACK_INSTALL_MODULE,
        package / "__init__.py",
        submodule_search_locations=[str(package)],
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("SugarCubes' update step could not be loaded.")
    module = importlib.util.module_from_spec(spec)
    sys.modules[_PACK_INSTALL_MODULE] = module
    try:
        spec.loader.exec_module(module)
        module.apply_pending_update(_installed_folder)
    finally:
        for name in [
            loaded
            for loaded in sys.modules
            if loaded == _PACK_INSTALL_MODULE
            or loaded.startswith(f"{_PACK_INSTALL_MODULE}.")
        ]:
            sys.modules.pop(name, None)


_apply_pending_pack_update()

_spec = importlib.util.spec_from_file_location(
    "sugarcubes_root_api_loader", _extension / "load_model_roots.py"
)
if _spec is None or _spec.loader is None:
    raise RuntimeError("SugarCubes' model-root loader could not be loaded.")
_loader = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_loader)
_api = _loader.load_model_root_api()

_api.apply_configured_model_root(importlib.import_module("folder_paths"))
