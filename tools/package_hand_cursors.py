# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Package the preserved Qt PNGs into native Windows CUR files without resampling.

Run from the repository root with Python and Pillow. Qt's original RGBA pixels
and per-size hotspots are preserved in uncompressed, bottom-up Windows DIBs.
"""

from pathlib import Path
import struct

from PIL import Image


ASSETS = Path(__file__).resolve().parents[1] / "crates/sugarsubstitute-desktop/assets/cursors"
HOTSPOTS = ((32, 10, 12), (48, 15, 16), (64, 20, 24))


def dib(source: Path, size: int) -> bytes:
    """Preserve RGBA pixels and supply the legacy transparent-pixel AND mask."""
    with Image.open(source) as image:
        if image.size != (size, size):
            raise ValueError(f"Unexpected cursor extent: {source}")
        rgba = image.convert("RGBA")
        pixels = rgba.load()
        color = bytearray()
        mask = bytearray()
        stride = ((size + 31) // 32) * 4
        for y in reversed(range(size)):
            row = bytearray(stride)
            for x in range(size):
                red, green, blue, alpha = pixels[x, y]
                color.extend((blue, green, red, alpha))
                if alpha == 0:
                    row[x // 8] |= 0x80 >> (x % 8)
            mask.extend(row)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0,
                         len(color) + len(mask), 0, 0, 0, 0)
    return header + color + mask


def package(hand: str) -> None:
    """Write three original resolutions with the hotspots used by Qt 6.11.0."""
    entries = bytearray()
    payload = bytearray()
    offset = 6 + 16 * len(HOTSPOTS)
    for size, x, y in HOTSPOTS:
        frame = dib(ASSETS / f"{hand}handcursor_{size}.png", size)
        entries.extend(struct.pack("<BBBBHHII", size, size, 0, 0, x, y,
                                   len(frame), offset + len(payload)))
        payload.extend(frame)
    (ASSETS / f"qt-{hand}-hand.cur").write_bytes(
        struct.pack("<HHH", 0, 2, len(HOTSPOTS)) + entries + payload)


if __name__ == "__main__":
    for name in ("open", "closed"):
        package(name)
