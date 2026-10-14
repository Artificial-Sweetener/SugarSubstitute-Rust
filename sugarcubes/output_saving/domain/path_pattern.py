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
"""Validate relative output patterns and preserve Substitute token rendering."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
import re

DEFAULT_PATH_PATTERN = r"{date}\{run}_{cube#}_{workflow}_{source}"
SUPPORTED_TOKENS = (
    "run",
    "cube#",
    "image#",
    "seed",
    "workflow",
    "source",
    "cube",
    "date",
    "time",
    "day",
    "width",
    "height",
    "index",
    "set",
    "node",
    "node#",
    "batch",
)
_TOKEN = re.compile(r"\{([^{}]+)\}")
_SEPARATOR = re.compile(r"[\\/]+")
_UNSAFE = re.compile(r'[<>:"/\\|?*\x00-\x1f]+')
_DEVICE = re.compile(r"^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)", re.I)


def validate_pattern(pattern: str) -> tuple[str, ...]:
    """Reject unknown tokens and absolute or traversing paths before filesystem use."""
    if not pattern.strip() or len(pattern) > 2048 or "\x00" in pattern:
        raise ValueError(
            "Output pattern must contain a filename and fit within 2048 characters."
        )
    if pattern.startswith(("/", "\\")) or re.match(r"^[a-zA-Z]:", pattern):
        raise ValueError("Output pattern must be relative to the output folder.")
    remainder = _TOKEN.sub("", pattern)
    if "{" in remainder or "}" in remainder:
        raise ValueError("Output pattern contains a malformed placeholder.")
    unknown = set(_TOKEN.findall(pattern)).difference(SUPPORTED_TOKENS)
    if unknown:
        raise ValueError(f"Unknown output tokens: {', '.join(sorted(unknown))}.")
    parts = tuple(_SEPARATOR.split(pattern.strip()))
    if any(not part or part.strip() in {".", ".."} for part in parts):
        raise ValueError(
            "Output pattern must contain valid relative folder and filename components."
        )
    return parts


@dataclass(frozen=True, slots=True)
class OutputNamingContext:
    """Carry exact per-image naming facts without depending on a canvas or filesystem."""

    workflow: str
    source: str
    started_at: datetime
    run: int | None = None
    cube: str = ""
    cube_number: int | None = None
    folder_image_number: int | None = None
    seed: str = ""
    width: int = 0
    height: int = 0
    index: int = 1
    set_index: int = 1
    node: str = ""
    node_id: str = ""

    def tokens(self) -> dict[str, str]:
        """Retain Substitute padding and one-based source image numbering."""
        return {
            "run": f"{self.run:03d}" if self.run is not None else "",
            "cube#": f"{self.cube_number:02d}" if self.cube_number is not None else "",
            "image#": (
                f"{self.folder_image_number:02d}"
                if self.folder_image_number is not None
                else ""
            ),
            "seed": self.seed,
            "workflow": self.workflow,
            "source": self.source,
            "cube": self.cube,
            "date": self.started_at.strftime("%Y-%m-%d"),
            "time": self.started_at.strftime("%H-%M-%S"),
            "day": self.started_at.strftime("%A"),
            "width": str(self.width),
            "height": str(self.height),
            "index": str(self.index),
            "set": str(self.set_index),
            "node": self.node,
            "node#": self.node_id,
            "batch": str(self.index),
        }


def render_components(pattern: str, context: OutputNamingContext) -> tuple[str, ...]:
    """Preserve folder case and spacing while normalizing filename stems like Substitute."""
    parts = validate_pattern(pattern)
    values = context.tokens()
    rendered: list[str] = []
    for index, part in enumerate(parts):
        filename = index == len(parts) - 1
        text = _TOKEN.sub(
            lambda match: _safe_component(values[match.group(1)], filename=filename),
            part,
        )
        rendered.append(
            _safe_component(text, filename=filename)
            or ("output" if filename else "untitled")
        )
    return tuple(rendered)


def run_bucket_pattern(pattern: str) -> str | None:
    """Find the parent namespace used to allocate Substitute's committed run number."""
    parts = validate_pattern(pattern)
    directories = parts[:-1]
    if "{run}" not in parts[-1]:
        directories = directories[
            : next(
                (i for i, part in enumerate(directories) if "{run}" in part),
                len(directories),
            )
        ]
    return "/".join(directories) or None


def _safe_component(value: str, *, filename: bool) -> str:
    """Keep token text inside one portable path component, including Windows device names."""
    text = _UNSAFE.sub("_", value.strip().lower() if filename else value.strip())
    if filename:
        text = re.sub(r"\s+", "_", text)
    text = re.sub(r"_+", "_", text).strip(" ._")
    if _DEVICE.match(text):
        text = f"_{text}"
    return text
