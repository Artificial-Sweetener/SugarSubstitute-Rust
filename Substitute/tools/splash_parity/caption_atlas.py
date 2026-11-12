# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Generate native caption coverage from the installed original MIT close SVG paths."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import struct
from pathlib import Path


def encode_path(width: int, height: int, alpha: bytes) -> bytes:
    """Encode nonzero coverage without resampling or changing compositing order."""
    if not 0 < width <= 65535 or not 0 < height <= 65535 or len(alpha) != width * height:
        raise ValueError("Invalid caption coverage dimensions")
    runs: list[bytes] = []
    for y in range(height):
        x = 0
        while x < width:
            if not alpha[y * width + x]:
                x += 1
                continue
            first = x
            while x < width and alpha[y * width + x]:
                x += 1
            runs.append(struct.pack("<HHH", y, first, x - first) + alpha[y * width + first:y * width + x])
    return struct.pack("<I", len(runs)) + b"".join(runs)


def encode_atlas(first_dpi: int, records: list[bytes]) -> bytes:
    """Index complete integer-DPI records in the bounded schema-one resource."""
    if not records or not 1 <= first_dpi <= first_dpi + len(records) - 1 <= 768:
        raise ValueError("Invalid caption atlas DPI range")
    offset = 16 + (len(records) + 1) * 4
    offsets = [offset]
    for record in records:
        offset += len(record)
        offsets.append(offset)
    return (
        struct.pack("<8sHHI", b"SSCAPA1\0", first_dpi, first_dpi + len(records) - 1, len(records))
        + struct.pack(f"<{len(offsets)}I", *offsets)
        + b"".join(records)
    )


def generate(output: Path) -> None:
    """Rasterize each original SVG path separately using native Qt and exact device scales.

    The original SVG contains two paths. Keeping their coverage separate preserves
    Qt's integer SourceOver rounding when painting on translucent or colored fills.
    Each indexed record starts with logical-button raster dimensions and the three
    fractional rectangle-edge coverages, followed by the two sparse SVG paths.
    No widget is shown and no desktop capture API is used.
    """
    os.environ["QT_QPA_PLATFORM"] = "windows"
    from PySide6 import __version__ as pyside_version
    from PySide6.QtCore import QPoint, Qt
    from PySide6.QtGui import QColor, QImage, QPainter, QRegion
    from PySide6.QtWidgets import QApplication, QWidget
    from PySide6.QtXml import QDomDocument
    from qframelesswindow.titlebar.title_bar_buttons import CloseButton, TitleBarButtonState

    application = QApplication.instance() or QApplication([])
    if application.platformName() != "windows":
        raise RuntimeError("Native Windows Qt is required for caption coverage")
    root = QWidget()
    root.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
    root.resize(46, 32)
    button = CloseButton(root)
    original = bytes(button._svgDom.toByteArray())
    paths: list[QDomDocument] = []
    for keep in (0, 1):
        document = QDomDocument()
        document.setContent(original)
        nodes = document.elementsByTagName("path")
        if nodes.count() != 2:
            raise ValueError("The licensed close SVG must contain exactly two original paths")
        remove = nodes.at(1 - keep)
        remove.parentNode().removeChild(remove)
        paths.append(document)
    button.setNormalColor(QColor(255, 255, 255))
    root.ensurePolished()
    button.ensurePolished()
    records: list[bytes] = []
    for dpi in range(48, 769):
        scale = dpi / 96
        width, height = math.floor(46 * scale + 0.5), math.floor(32 * scale + 0.5)
        button.setState(TitleBarButtonState.HOVER)
        background = QImage(math.ceil(46 * scale), math.ceil(32 * scale),
                            QImage.Format.Format_ARGB32_Premultiplied)
        background.setDevicePixelRatio(scale)
        background.fill(Qt.GlobalColor.transparent)
        painter = QPainter(background)
        root.render(painter, QPoint(), QRegion(), root.RenderFlag.DrawChildren)
        painter.end()
        coverage = bytes(background.constBits())[3::4]
        record = struct.pack("<HHBBB", width, height, coverage[background.width() - 1],
                             coverage[(background.height() - 1) * background.width()], coverage[-1])
        button.setState(TitleBarButtonState.NORMAL)
        for document in paths:
            button._svgDom = document
            image = QImage(width, height, QImage.Format.Format_ARGB32_Premultiplied)
            image.setDevicePixelRatio(scale)
            image.fill(Qt.GlobalColor.transparent)
            painter = QPainter(image)
            root.render(painter, QPoint(), QRegion(), root.RenderFlag.DrawChildren)
            painter.end()
            record += encode_path(width, height, bytes(image.constBits())[3::4])
        records.append(record)
    encoded = encode_atlas(48, records)
    if len(encoded) > 1024 * 1024:
        raise ValueError("Caption coverage exceeds its one-MiB packaged resource bound")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    output.with_suffix(".json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "source_url": "https://github.com/zhiyiYo/PyQt-Frameless-Window",
                "source_svg_sha256": hashlib.sha256(original).hexdigest(),
                "pyside_version": pyside_version,
                "qt_platform": application.platformName(),
                "first_dpi": 48,
                "last_dpi": 768,
                "record_count": len(records),
                "byte_length": len(encoded),
                "sha256": hashlib.sha256(encoded).hexdigest(),
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    button.close()
    root.close()


def main() -> None:
    """Generate only the explicitly named resource and its provenance receipt."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    generate(parser.parse_args().output)


if __name__ == "__main__":
    main()
