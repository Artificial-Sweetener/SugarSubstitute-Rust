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
"""Bind Comfy-created output node instances to one explicit host-owned image saving port."""

from __future__ import annotations

import sys
from typing import Protocol

from ..runtime.cube_output_events import CubeOutputArtifact


class RuntimeImageSavingPort(Protocol):
    """Expose only image materialization to Comfy nodes created outside dependency injection."""

    def save(
        self, value: object, ticket_id: str
    ) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
        """Materialize one ticketed image value and return neutral artifact identities."""


_ACTIVE_IMAGES: RuntimeImageSavingPort | None = None


def bind_runtime_images(images: RuntimeImageSavingPort) -> None:
    """Install the host composition's single image port without constructing fallback services."""
    global _ACTIVE_IMAGES
    if _ACTIVE_IMAGES is not None and _ACTIVE_IMAGES is not images:
        raise RuntimeError(
            "Output saving is already bound to another active Comfy host."
        )
    _ACTIVE_IMAGES = images


def release_runtime_images(images: RuntimeImageSavingPort) -> None:
    """Release only the port owned by the shutting-down host."""
    global _ACTIVE_IMAGES
    if _ACTIVE_IMAGES is images:
        _ACTIVE_IMAGES = None


def runtime_images() -> RuntimeImageSavingPort:
    """Fail explicitly when a ticketed sink runs without its authoritative host service."""
    if _ACTIVE_IMAGES is None:
        raise RuntimeError(
            "SugarCubes output saving is unavailable in this Comfy host."
        )
    return _ACTIVE_IMAGES


sys.modules["sugarcubes.output_saving.host_binding"] = sys.modules[__name__]
