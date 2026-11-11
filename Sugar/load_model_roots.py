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
"""Publish the lightweight model-root API for Comfy prestartup and offline host tools."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
from types import ModuleType


def load_model_root_api() -> ModuleType:
    """Load the public package once without importing SugarCubes' node or execution modules."""
    name = "sugarcubes_model_roots"
    existing = sys.modules.get(name)
    if existing is not None:
        if getattr(existing, "MODEL_ROOT_API_VERSION", None) != 1:
            raise RuntimeError("The loaded SugarCubes model-root API is unsupported.")
        return existing
    package = Path(__file__).resolve().parent / name
    spec = importlib.util.spec_from_file_location(
        name, package / "__init__.py", submodule_search_locations=[str(package)]
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("SugarCubes' model-root API could not be loaded.")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(name, None)
        raise
    return module
