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

"""Adapt Comfy video values into fetchable cube output artifacts."""

from __future__ import annotations

import io
import shutil
import uuid
from pathlib import Path
from typing import Protocol, cast, runtime_checkable

from .cube_output_events import CubeOutputArtifact

_MIME_TYPES = {
    ".avi": "video/x-msvideo",
    ".gif": "image/gif",
    ".mkv": "video/x-matroska",
    ".mov": "video/quicktime",
    ".mp4": "video/mp4",
    ".webm": "video/webm",
    ".webp": "image/webp",
}


@runtime_checkable
class _VideoValue(Protocol):
    """Describe the stable Comfy video input surface used by output adaptation."""

    def get_stream_source(self) -> str | io.BytesIO:
        """Return a file or encoded buffer suitable for streaming."""

    def get_active_trim_window(self) -> tuple[float, float]:
        """Return the active start and duration in seconds."""

    def get_dimensions(self) -> tuple[int, int]:
        """Return video dimensions."""

    def get_duration(self) -> float:
        """Return the active video duration in seconds."""

    def save_to(self, path: str) -> object:
        """Encode the active video value to one file."""


class _FolderPaths(Protocol):
    """Describe the Comfy storage roots needed for artifact addressing."""

    def get_input_directory(self) -> str:
        """Return the configured Comfy input directory."""

    def get_output_directory(self) -> str:
        """Return the configured Comfy output directory."""

    def get_temp_directory(self) -> str:
        """Return the configured Comfy temporary directory."""


def build_video_output_preview(
    value: object,
) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
    """Return Comfy UI metadata and a fetchable artifact for a video value."""

    if not isinstance(value, _VideoValue):
        return {}, ()
    source = value.get_stream_source()
    start_time, duration = value.get_active_trim_window()
    locator = None
    if start_time == 0.0 and duration == 0.0 and isinstance(source, str):
        locator = _comfy_locator(Path(source))
    if locator is None:
        locator = _materialize_video(
            value,
            source=source,
            preserve_source=start_time == 0.0 and duration == 0.0,
        )
    artifact_type, filename, subfolder = locator
    width, height = _positive_dimensions(value.get_dimensions())
    duration_seconds = _positive_duration(value.get_duration())
    artifact = CubeOutputArtifact(
        filename=filename,
        subfolder=subfolder,
        type=artifact_type,
        media_kind="video",
        mime_type=_MIME_TYPES.get(Path(filename).suffix.lower()),
        width=width,
        height=height,
        duration_seconds=duration_seconds,
    )
    reference = {
        "filename": filename,
        "subfolder": subfolder,
        "type": artifact_type,
    }
    return {"images": [reference], "animated": (True,)}, (artifact,)


def _comfy_locator(path: Path) -> tuple[str, str, str] | None:
    """Map one existing file to a safe Comfy ``/view`` locator."""

    import folder_paths

    storage = cast(_FolderPaths, folder_paths)
    try:
        source = path.resolve(strict=True)
    except OSError:
        return None
    roots = (
        ("input", Path(storage.get_input_directory())),
        ("output", Path(storage.get_output_directory())),
        ("temp", Path(storage.get_temp_directory())),
    )
    for artifact_type, root in roots:
        try:
            relative = source.relative_to(root.resolve(strict=True))
        except (OSError, ValueError):
            continue
        subfolder = "" if relative.parent == Path(".") else relative.parent.as_posix()
        return artifact_type, relative.name, subfolder
    return None


def _materialize_video(
    value: _VideoValue,
    *,
    source: str | io.BytesIO,
    preserve_source: bool,
) -> tuple[str, str, str]:
    """Persist non-addressable or trimmed video data under Comfy's temp root."""

    import folder_paths

    storage = cast(_FolderPaths, folder_paths)
    temp_root = Path(storage.get_temp_directory())
    temp_root.mkdir(parents=True, exist_ok=True)
    suffix = _video_suffix(source)
    filename = f"SugarCubes_video_{uuid.uuid4().hex}{suffix}"
    destination = temp_root / filename
    if preserve_source and isinstance(source, io.BytesIO):
        position = source.tell()
        try:
            source.seek(0)
            destination.write_bytes(source.read())
        finally:
            source.seek(position)
    elif preserve_source and isinstance(source, str) and Path(source).is_file():
        shutil.copyfile(source, destination)
    else:
        value.save_to(str(destination))
    return "temp", filename, ""


def _video_suffix(source: str | io.BytesIO) -> str:
    """Return a supported source suffix or the core Comfy MP4 default."""

    if isinstance(source, str):
        suffix = Path(source).suffix.lower()
        if suffix in _MIME_TYPES:
            return suffix
    return ".mp4"


def _positive_dimensions(dimensions: tuple[int, int]) -> tuple[int | None, int | None]:
    """Normalize invalid media dimensions to absent metadata."""

    width, height = dimensions
    return (width if width > 0 else None, height if height > 0 else None)


def _positive_duration(duration: float) -> float | None:
    """Normalize invalid media duration to absent metadata."""

    return float(duration) if duration > 0 else None


__all__ = ["build_video_output_preview"]
