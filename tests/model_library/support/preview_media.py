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
"""Encode distinct animation frames to prove thumbnails retain the first visual frame."""

from io import BytesIO

import av
from PIL import Image


def animated_preview(format: str) -> bytes:
    """Build a red-first, blue-second GIF or video entirely in memory."""
    target = BytesIO()
    width, height = 64, 96
    if format == "gif":
        first = Image.new("RGB", (width, height), "red")
        second = Image.new("RGB", (width, height), "blue")
        first.save(target, format="GIF", save_all=True, append_images=[second])
    else:
        with av.open(target, mode="w", format=format) as container:
            stream = container.add_stream(
                "mpeg4" if format == "mp4" else "libvpx", rate=2
            )
            stream.width, stream.height = width, height
            stream.pix_fmt = "yuv420p"
            for color in (b"\xff\x00\x00\xff", b"\x00\x00\xff\xff"):
                frame = av.VideoFrame.from_bytes(
                    color * width * height, width, height, "rgba"
                )
                for packet in stream.encode(frame):
                    container.mux(packet)
            for packet in stream.encode():
                container.mux(packet)
    return target.getvalue()
