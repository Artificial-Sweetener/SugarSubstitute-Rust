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
"""Expose the versioned, process-local API consumed by sibling extensions."""

from __future__ import annotations

import sys

from .backend.composition import BackendServices
from .runtime import (
    register_cube_output_observer,
    unregister_cube_output_observer,
)

HOST_API_VERSION = 1
HOST_API_MODULE_NAME = "sugarcubes.host_api"

_ACTIVE_BACKEND_SERVICES: BackendServices | None = None


def set_active_backend_services(services: BackendServices) -> None:
    """Publish the service graph created by the ComfyUI extension entrypoint."""

    global _ACTIVE_BACKEND_SERVICES
    _ACTIVE_BACKEND_SERVICES = services


def active_backend_services() -> BackendServices | None:
    """Return the active service graph without constructing a duplicate graph."""

    return _ACTIVE_BACKEND_SERVICES


def _publish_canonical_module_identity() -> None:
    """Publish one stable identity despite ComfyUI's path-derived module names."""

    sys.modules[HOST_API_MODULE_NAME] = sys.modules[__name__]


_publish_canonical_module_identity()

__all__ = [
    "HOST_API_MODULE_NAME",
    "HOST_API_VERSION",
    "active_backend_services",
    "register_cube_output_observer",
    "set_active_backend_services",
    "unregister_cube_output_observer",
]
