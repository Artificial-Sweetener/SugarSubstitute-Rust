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
"""Define portable artifact names without accepting executable or path-bearing filenames."""

from __future__ import annotations

import re


def safe_artifact_name(name: str) -> bool:
    """Reject traversal, Windows device names, alternate streams, and executable extensions."""
    reserved = {
        "CON",
        "PRN",
        "AUX",
        "NUL",
        *(f"COM{i}" for i in range(1, 10)),
        *(f"LPT{i}" for i in range(1, 10)),
    }
    return (
        0 < len(name) <= 200
        and not re.search(r'[<>:"/\\|?*\x00-\x1f]', name)
        and name == name.strip(" .")
        and name not in (".", "..")
        and name.split(".")[0].upper() not in reserved
        and name.lower().endswith((".safetensors", ".ckpt", ".pt", ".pth", ".bin"))
    )
