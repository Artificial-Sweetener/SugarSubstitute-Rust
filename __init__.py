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
"""SugarCubes ComfyUI extension entry point."""

from __future__ import annotations

import logging
import sys
from pathlib import Path

try:
    from .backend import build_backend_services, set_active_backend_services
    from .backend.routes import register_routes
    from .exporter import (
        CubeValidationError,
        DefinitionResolver,
        ExportedCube,
        export as export_cubes,
        write_cube,
        write_cubes,
    )
    from .nodes import NODE_CLASS_MAPPINGS, NODE_DISPLAY_NAME_MAPPINGS
except ImportError:
    from backend import build_backend_services, set_active_backend_services
    from backend.routes import register_routes
    from exporter import (
        CubeValidationError,
        DefinitionResolver,
        ExportedCube,
        export as export_cubes,
        write_cube,
        write_cubes,
    )
    from nodes import NODE_CLASS_MAPPINGS, NODE_DISPLAY_NAME_MAPPINGS

WEB_DIRECTORY = "web"
__version__ = "0.10.0"
_EXTENSION_ROOT = Path(__file__).resolve().parent
_LOGGER = logging.getLogger(__name__)

_prompt_server_module = sys.modules.get("server")
PromptServer = (
    getattr(_prompt_server_module, "PromptServer", None)
    if _prompt_server_module is not None
    else None
)
if PromptServer is not None:
    try:
        _backend_services = build_backend_services(_EXTENSION_ROOT)
        set_active_backend_services(_backend_services)
        register_routes(PromptServer, _backend_services)
    except (
        AttributeError,
        ImportError,
        OSError,
        RuntimeError,
        TypeError,
        ValueError,
    ):  # pragma: no cover - depends on Comfy import host state
        _LOGGER.exception(
            "ERROR: SugarCubes: backend route registration failed; cube library APIs "
            "may be unavailable."
        )


__all__ = [
    "NODE_CLASS_MAPPINGS",
    "NODE_DISPLAY_NAME_MAPPINGS",
    "WEB_DIRECTORY",
    "__version__",
    "CubeValidationError",
    "DefinitionResolver",
    "ExportedCube",
    "export_cubes",
    "write_cube",
    "write_cubes",
]
