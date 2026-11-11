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
"""Apply SugarCubes-owned model-root configuration before normal Comfy custom-node imports."""

from __future__ import annotations

import importlib
import importlib.util
from pathlib import Path

_extension = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location(
    "sugarcubes_root_api_loader", _extension / "load_model_roots.py"
)
if _spec is None or _spec.loader is None:
    raise RuntimeError("SugarCubes' model-root loader could not be loaded.")
_loader = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_loader)
_api = _loader.load_model_root_api()

_api.apply_configured_model_root(importlib.import_module("folder_paths"))
