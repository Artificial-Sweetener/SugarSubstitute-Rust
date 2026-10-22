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
"""Generate tiny original media fixtures for browser qualification without reading user media."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import struct

import av
import numpy as np
from PIL import Image, ImageDraw


def generate_synthetic_media(root: Path) -> None:
    """Write a still, animation, short video and self-contained colored tetrahedron."""
    root.mkdir(parents=True, exist_ok=True)
    images: list[Image.Image] = []
    for index in range(24):
        image = Image.new("RGB", (512, 320), (18, 24, 42))
        draw = ImageDraw.Draw(image)
        draw.rectangle(
            (20 + index * 12, 65, 120 + index * 12, 245), fill=(245, 70, 160)
        )
        draw.ellipse((285, 60, 455, 230), fill=(50, 210, 215))
        draw.text((24, 24), f"SYNTHETIC OUTPUT / FRAME {index:02}", fill="white")
        images.append(image)
    images[0].save(root / "still.png")
    images[0].save(
        root / "animation.gif",
        save_all=True,
        append_images=images[1::4],
        duration=160,
        loop=0,
    )
    with av.open(str(root / "clip.webm"), "w") as container:
        stream = container.add_stream("libvpx-vp9", rate=12)
        stream.width = 512
        stream.height = 320
        stream.pix_fmt = "yuv420p"
        for image in images:
            frame = av.VideoFrame.from_ndarray(np.array(image), format="rgb24")
            for packet in stream.encode(frame):
                container.mux(packet)
        for packet in stream.encode():
            container.mux(packet)
    vertices = struct.pack("<12f", 0, 1.2, 0, -1, -0.7, 0.8, 1, -0.7, 0.8, 0, -0.7, -1)
    indices = struct.pack("<12H", 0, 1, 2, 0, 2, 3, 0, 3, 1, 1, 3, 2)
    data = vertices + indices
    model = {
        "asset": {"version": "2.0", "generator": "SugarCubes synthetic qualification"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [
            {
                "primitives": [
                    {"attributes": {"POSITION": 0}, "indices": 1, "material": 0}
                ]
            }
        ],
        "materials": [
            {
                "doubleSided": True,
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.1, 0.8, 0.85, 1],
                    "metallicFactor": 0,
                    "roughnessFactor": 0.6,
                },
            }
        ],
        "buffers": [{"byteLength": len(data)}],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": 0,
                "byteLength": len(vertices),
                "target": 34962,
            },
            {
                "buffer": 0,
                "byteOffset": len(vertices),
                "byteLength": len(indices),
                "target": 34963,
            },
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 4,
                "type": "VEC3",
                "min": [-1, -0.7, -1],
                "max": [1, 1.2, 0.8],
            },
            {"bufferView": 1, "componentType": 5123, "count": 12, "type": "SCALAR"},
        ],
    }
    metadata = json.dumps(model).encode()
    metadata += b" " * (-len(metadata) % 4)
    data += b"\0" * (-len(data) % 4)
    content = (
        struct.pack("<II", len(metadata), 0x4E4F534A)
        + metadata
        + struct.pack("<II", len(data), 0x004E4942)
        + data
    )
    (root / "tetrahedron.glb").write_bytes(
        struct.pack("<III", 0x46546C67, 2, 12 + len(content)) + content
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    generate_synthetic_media(parser.parse_args().root)
