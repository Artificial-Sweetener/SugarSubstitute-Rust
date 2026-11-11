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
"""Probe original media through an already qualified handle under a cumulative metadata budget."""

from dataclasses import dataclass
from io import BufferedIOBase
import json
from typing import BinaryIO, Literal, cast

from PIL import Image, UnidentifiedImageError

from ...model_library.infrastructure.preview_frame import probe_video_dimensions
from ..domain.artifact import ArtifactUnavailable
from .artifact_budget import ArtifactBudget


class MetadataReader(BufferedIOBase):
    """Expose a bounded seekable view without closing the file handle owned by the receipt worker."""

    def __init__(self, stream: BinaryIO, budget: ArtifactBudget) -> None:
        """Borrow the pinned file and the requesting operation's cumulative admission."""
        super().__init__()
        self._stream = stream
        self._budget = budget

    def read(self, size: int | None = -1) -> bytes:
        """Refuse unbounded metadata allocations before calling the underlying file."""
        if size is None or size < 0:
            position = self._stream.tell()
            size = self._stream.seek(0, 2) - position
            self._stream.seek(position)
        if size > self._budget.maximum_metadata:
            raise ArtifactUnavailable(
                "budget", "Image metadata exceeds the inspection limit."
            )
        self._budget.metadata(size)
        return self._stream.read(size)

    def seek(self, offset: int, whence: int = 0) -> int:
        """Keep arbitrary file positions inside the existing pinned stream."""
        self._budget.check()
        return self._stream.seek(offset, whence)

    def tell(self) -> int:
        """Report the borrowed stream's current offset."""
        return self._stream.tell()

    def readable(self) -> bool:
        """Permit header parsers to use this read-only file view."""
        return True

    def seekable(self) -> bool:
        """Permit bounded metadata parsers to revisit the original header."""
        return True


@dataclass(frozen=True, slots=True)
class ArtifactProbe:
    """Return exact media header facts without assigning workflow or runtime presentation identity."""

    width: int
    height: int
    mime: str
    media: Literal["image", "video"]
    embedded: bool = False
    prompt_id: str | None = None
    animated: bool = False


def image_metadata(
    stream: BinaryIO, budget: ArtifactBudget, suffix: str
) -> ArtifactProbe:
    """Probe supported original media through the same stable handle with finite metadata admission."""
    stream.seek(0)
    reader = MetadataReader(stream, budget)
    bounded = cast(BinaryIO, reader)
    if suffix in {".mp4", ".webm"}:
        try:
            width, height = probe_video_dimensions(
                bounded, "mov" if suffix == ".mp4" else "matroska"
            )
            budget.check()
            return ArtifactProbe(
                width,
                height,
                "video/mp4" if suffix == ".mp4" else "video/webm",
                "video",
            )
        except ArtifactUnavailable:
            raise
        except (ValueError, OSError) as error:
            budget.check()
            raise ArtifactUnavailable(
                "unsupported", "The output video header is invalid."
            ) from error
    try:
        with Image.open(bounded) as image:
            mime = {
                "PNG": "image/png",
                "JPEG": "image/jpeg",
                "WEBP": "image/webp",
                "GIF": "image/gif",
            }.get(image.format or "")
            required = {
                ".png": "image/png",
                ".apng": "image/png",
                ".jpg": "image/jpeg",
                ".jpeg": "image/jpeg",
                ".webp": "image/webp",
                ".gif": "image/gif",
            }.get(suffix)
            if (
                mime is None
                or mime != required
                or image.width < 1
                or image.height < 1
                or image.width > 131072
                or image.height > 131072
            ):
                raise ArtifactUnavailable(
                    "unsupported", "This output image format is unsupported."
                )
            raw: object = image.info.get("sugarcubes_output")
            embedded, prompt = _embedded_identity(raw)
            # GIF uses the existing animated presentation owner even for a single-frame container.
            animated = (
                image.format == "GIF"
                or getattr(image, "custom_mimetype", None) == "image/apng"
            )
            if image.format == "WEBP":
                reader.seek(0)
                prefix = reader.read(21)
                animated = (
                    len(prefix) == 21
                    and prefix[12:16] == b"VP8X"
                    and bool(prefix[20] & 0x02)
                )
            return ArtifactProbe(
                image.width, image.height, mime, "image", embedded, prompt, animated
            )
    except (UnidentifiedImageError, Image.DecompressionBombError, OSError) as error:
        raise ArtifactUnavailable(
            "unsupported", "The output image header is invalid."
        ) from error


def _embedded_identity(value: object) -> tuple[bool, str | None]:
    """Recognize supported metadata without treating recipe JSON or node numbers as workflow custody."""
    if value is None:
        return False, None
    if not isinstance(value, str) or len(value.encode("utf-8")) > 1048576:
        raise ArtifactUnavailable(
            "unsupported", "The output identity metadata is invalid."
        )
    try:
        decoded: object = json.loads(value)
    except ValueError as error:
        raise ArtifactUnavailable(
            "unsupported", "The output identity metadata is invalid."
        ) from error
    if not isinstance(decoded, dict) or decoded.get("schema_version") != 1:
        raise ArtifactUnavailable(
            "unsupported", "The output identity metadata version is unsupported."
        )
    prompt: object = decoded.get("run_id")
    if not isinstance(prompt, str) or not prompt or len(prompt.encode("utf-8")) > 256:
        raise ArtifactUnavailable(
            "unsupported", "The output identity metadata is invalid."
        )
    return True, prompt
