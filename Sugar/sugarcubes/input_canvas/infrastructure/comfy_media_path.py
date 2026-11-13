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
"""Validate annotated source files against Comfy's declared media directories."""

from __future__ import annotations
from importlib import import_module
from pathlib import Path


def validate_input_file(file: str) -> Path:
    """Constrain annotated Comfy filenames to their declared input, output or temporary directory."""
    if not file or len(file) > 4096 or "\x00" in file:
        raise ValueError("Invalid input source file.")
    folders = import_module("folder_paths")
    annotate = getattr(folders, "annotated_filepath", None)
    directory = getattr(folders, "get_input_directory", None)
    if not callable(annotate) or not callable(directory):
        raise RuntimeError("Comfy's input file resolver is unavailable.")
    annotated: object = annotate(file)
    if not isinstance(annotated, tuple) or len(annotated) != 2:
        raise RuntimeError("Comfy returned an invalid input file reference.")
    filename: object = annotated[0]
    root_value: object = annotated[1] if annotated[1] is not None else directory()
    if not isinstance(filename, str) or not isinstance(root_value, str):
        raise RuntimeError("Comfy returned an invalid input directory.")
    root = Path(root_value).resolve()
    candidate = (root / filename).resolve()
    if not candidate.is_relative_to(root):
        raise ValueError(
            "Input source files must remain inside the Comfy media directory."
        )
    if not candidate.is_file():
        raise FileNotFoundError("The input source file is unavailable.")
    return candidate
