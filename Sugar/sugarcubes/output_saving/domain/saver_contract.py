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
"""Describe explicitly qualified image passthrough sinks independently of host classes."""

from dataclasses import dataclass

from .run_plan import SourceKind


@dataclass(frozen=True, slots=True)
class ImageSaverContract:
    """Authorize replacement of a declared saver whose only result is its unchanged image input.

    A custom node author opts into this contract with
    ``SUGARCUBES_IMAGE_SAVER = {"schema_version": 1, "image_input": "images",
    "kind": "save", "mode": "replace_image_passthrough"}``. Such a node must
    expose one IMAGE result, return the input unchanged, and delegate all its
    saving side effects when managed. Additional transformations, results or
    unrelated side effects require a separate qualified adapter.
    """

    class_type: str
    image_input: str
    kind: SourceKind
