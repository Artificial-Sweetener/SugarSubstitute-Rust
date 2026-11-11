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
"""Decode one bounded visual frame for portable model-thumbnail preparation."""

from io import BytesIO
from typing import BinaryIO, Literal

import av
from PIL import Image, ImageOps

_PIXEL_LIMIT = 40_000_000


def first_preview_frame(content: bytes) -> Image.Image:
    """Return a detached RGB first frame from an image, GIF, MP4, or WebM source."""
    video_format = (
        "mov"
        if content[4:8] == b"ftyp"
        else "matroska" if content.startswith(b"\x1a\x45\xdf\xa3") else None
    )
    if video_format is not None:
        return _video_frame(content, video_format)
    with Image.open(BytesIO(content)) as opened:
        _validate_dimensions(opened.width, opened.height)
        opened.seek(0)
        return ImageOps.exif_transpose(opened).convert("RGB")


def _video_frame(content: bytes, format: str) -> Image.Image:
    """Decode only the first video frame from an in-memory, explicitly selected container."""
    try:
        with av.open(BytesIO(content), mode="r", format=format) as container:
            if not container.streams.video:
                raise ValueError("Model preview contains no video frames.")
            stream = container.streams.video[0]
            _validate_dimensions(
                stream.codec_context.width, stream.codec_context.height
            )
            frame = next(container.decode(stream), None)
            if frame is None:
                raise ValueError("Model preview contains no video frames.")
            _validate_dimensions(frame.width, frame.height)
            rgb = frame.reformat(format="rgb24")
            plane = rgb.planes[0]
            return Image.frombytes(
                "RGB",
                (rgb.width, rgb.height),
                bytes(plane),
                "raw",
                "RGB",
                plane.line_size,
                1,
            )
    except av.error.FFmpegError as error:
        raise ValueError(
            "The model preview's first video frame could not be decoded."
        ) from error


def _validate_dimensions(width: int, height: int) -> None:
    """Reject invalid or oversized visual frames before converting pixel buffers."""
    if width <= 0 or height <= 0 or width * height > _PIXEL_LIMIT:
        raise ValueError("Model preview exceeds the supported pixel limit.")


def probe_video_dimensions(
    source: BinaryIO, format: Literal["mov", "matroska"]
) -> tuple[int, int]:
    """Inspect one explicit video container through its caller-bounded reader without retaining decoded frames."""
    try:
        with av.open(
            source,
            mode="r",
            format=format,
            options={
                "probesize": "1048576",
                "analyzeduration": "1000000",
                "threads": "1",
            },
        ) as container:
            if not container.streams.video:
                raise ValueError("Output media contains no video stream.")
            stream = container.streams.video[0]
            width, height = stream.codec_context.width, stream.codec_context.height
            _validate_dimensions(width, height)
            return width, height
    except av.error.FFmpegError as error:
        raise ValueError("The output video header could not be inspected.") from error
