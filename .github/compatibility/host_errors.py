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
"""Identify Python exceptions whose recorded stack actually entered the installed pack."""

import re
from pathlib import Path


def sugar_tracebacks(log: Path) -> dict[tuple[str, tuple[str, ...]], int]:
    """Retain the first complete stack for each distinct pack error in the real host log."""
    failures: dict[tuple[str, tuple[str, ...]], int] = {}
    with log.open(encoding="utf-8-sig", errors="replace") as stream:
        start = 0
        frames: list[str] = []
        for number, raw in enumerate(stream, 1):
            line = raw.rstrip()
            if line == "Traceback (most recent call last):":
                start = number
                frames = []
            elif start and (frame := re.match(r'^\s+File "([^"]+)", line (\d+)', line)):
                path = frame[1].replace("\\", "/")
                if "/custom_nodes/SugarCubes/" in path:
                    relative = "Sugar/" + path.partition("/custom_nodes/SugarCubes/")[2]
                    frames.append(f"{relative}:{frame[2]}")
                elif core := re.search(r"/comfy-[^/]+/comfy/(.+)$", path):
                    frames.append(f"ComfyUI/{core[1]}:{frame[2]}")
            elif start and line and not line[0].isspace():
                if any(frame.startswith("Sugar/") for frame in frames) and re.match(
                    r"^[\w.]+(?::|$)", line
                ):
                    failures.setdefault((line, tuple(frames)), start)
                start = 0
    return failures
