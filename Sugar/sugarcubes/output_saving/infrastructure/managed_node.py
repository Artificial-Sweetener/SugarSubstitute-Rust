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
"""Expose a prompt-only managed image sink while preserving Comfy image passthrough semantics."""

from __future__ import annotations

from ..host_binding import runtime_images


class ManagedImageOutput:
    """Delegate supported save-node persistence to a captured server-issued ticket."""

    CATEGORY = "SugarCubes/Internal"
    RETURN_TYPES = ("IMAGE",)
    RETURN_NAMES = ("images",)
    OUTPUT_NODE = True
    FUNCTION = "save_images"
    DEPRECATED = True

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, dict[str, tuple[object, ...]]]:
        """Require tensor input and a fresh ticket, invalidating only the output sink cache."""
        return {
            "required": {
                "images": ("IMAGE",),
                "save_ticket": ("STRING", {"default": ""}),
            }
        }

    def save_images(self, images: object, save_ticket: str) -> dict[str, object]:
        """Preserve downstream image values and native UI image controls."""
        ui, _artifacts = runtime_images().save(images, save_ticket)
        return {"ui": ui, "result": (images,)}
