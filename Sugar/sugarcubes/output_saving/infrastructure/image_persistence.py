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
"""Write canonical PNGs and optional JPEG companions into exclusively reserved contained paths."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import logging
from pathlib import Path
from typing import BinaryIO

from PIL import Image
from PIL.PngImagePlugin import PngInfo

from ..domain.path_pattern import OutputNamingContext, render_components
from ..domain.preferences import JpegPreferences
from .jpeg_encoder import JpegCompanionEncoder

_LOGGER = logging.getLogger(__name__)
_MAX_COLLISIONS = 10_000


@dataclass(frozen=True, slots=True)
class PersistedImageFiles:
    """Return committed image paths only after the canonical write has succeeded."""

    png: Path
    jpeg: Path | None


class ImageFilePersistence:
    """Own destination containment, exclusive collision handling and image writes."""

    def __init__(self, root: Path) -> None:
        """Require an explicit absolute host-approved destination root."""
        if not root.is_absolute():
            raise ValueError("Output folder must be an absolute server path.")
        if root.exists() and not root.is_dir():
            raise ValueError("Output folder must be a directory.")
        self._root = root.resolve()
        self._jpeg = JpegCompanionEncoder()

    def save(
        self,
        image: Image.Image,
        *,
        pattern: str,
        context: OutputNamingContext,
        metadata: Mapping[str, str],
        jpeg: JpegPreferences,
    ) -> PersistedImageFiles:
        """Commit PNG pixels and recipe metadata, preserving PNG on optional JPEG failure."""
        components = render_components(pattern, context)
        destination = self._contained(
            self._root.joinpath(*components[:-1], f"{components[-1]}.png")
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        path, png_stream, jpeg_stream = self._reserve(destination, jpeg.enabled)
        png_succeeded = False
        companion: Path | None = None
        try:
            info = PngInfo()
            for key, value in metadata.items():
                info.add_text(key, value)
            image.save(png_stream, format="PNG", pnginfo=info, compress_level=4)
            png_stream.flush()
            png_stream.close()
            png_succeeded = True
            if jpeg_stream is not None:
                try:
                    jpeg_stream.write(self._jpeg.encode(image, jpeg))
                    jpeg_stream.flush()
                    jpeg_stream.close()
                    companion = path.with_suffix(".jpg")
                except (OSError, ValueError, RuntimeError):
                    _LOGGER.exception(
                        "Optional JPEG companion failed; canonical PNG retained.",
                        extra={"output_filename": path.name},
                    )
            return PersistedImageFiles(path, companion)
        except (OSError, ValueError, RuntimeError):
            _LOGGER.exception(
                "Canonical output PNG could not be saved.",
                extra={"output_filename": path.name},
            )
            raise
        finally:
            png_stream.close()
            if jpeg_stream is not None:
                jpeg_stream.close()
                if companion is None:
                    path.with_suffix(".jpg").unlink(missing_ok=True)
            if not png_succeeded:
                path.unlink(missing_ok=True)

    def _reserve(
        self, destination: Path, companion: bool
    ) -> tuple[Path, BinaryIO, BinaryIO | None]:
        """Reserve both representations with exclusive creation, never overwriting an existing file."""
        canonical_collision = destination.exists()
        for ordinal in range(1, _MAX_COLLISIONS):
            ordinal_text = f"{ordinal:03d}" if canonical_collision else str(ordinal)
            candidate = (
                destination
                if ordinal == 1
                else destination.with_name(f"{destination.stem}_{ordinal_text}.png")
            )
            candidate = self._contained(candidate)
            jpeg_path = self._contained(candidate.with_suffix(".jpg"))
            if companion and jpeg_path.exists():
                continue
            try:
                png_stream = candidate.open("xb")
            except FileExistsError:
                canonical_collision = True
                continue
            except PermissionError:
                if candidate.exists():
                    canonical_collision = True
                    continue
                raise
            if not companion:
                return candidate, png_stream, None
            try:
                return candidate, png_stream, jpeg_path.open("xb")
            except FileExistsError:
                png_stream.close()
                candidate.unlink()
            except OSError:
                png_stream.close()
                candidate.unlink()
                raise
        raise FileExistsError("Output filename collision limit exceeded.")

    def _contained(self, path: Path) -> Path:
        """Resolve symlinks before permitting writes inside the configured root."""
        resolved = path.resolve()
        if not resolved.is_relative_to(self._root):
            raise ValueError("Output path escapes the configured output folder.")
        return resolved
