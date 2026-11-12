# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Isolate Qt's production white sweep gradient from source-over compositing."""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from pathlib import Path


def render(output: Path) -> None:
    """Export the actual QLinearGradient stops on transparent pixels without opening a window."""
    if sys.platform == "win32":
        os.environ["QT_QPA_PLATFORM"] = "windows"
    from PySide6.QtCore import QRectF
    from PySide6.QtGui import QColor, QGuiApplication, QImage, QLinearGradient, QPainter

    application = QGuiApplication.instance() or QGuiApplication([])
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    for scale in (1.0, 1.25, 1.5, 1.75, 2.0):
        size = math.floor(558 * scale + 0.5)
        image = QImage(size, size, QImage.Format.Format_ARGB32_Premultiplied)
        image.setDevicePixelRatio(scale)
        image.fill(QColor(0, 0, 0, 0))
        painter = QPainter(image)
        painter.translate(24, 356)
        gradient = QLinearGradient(102 - 71.4, 0, 102 + 71.4, 0)
        gradient.setColorAt(0, QColor(255, 255, 255, 0))
        gradient.setColorAt(0.5, QColor(255, 255, 255, 65))
        gradient.setColorAt(1, QColor(255, 255, 255, 0))
        painter.fillRect(QRectF(0, 0, 204, 6), gradient)
        painter.end()
        name = f"gradient-{scale:g}"
        if not image.save(str(output / f"{name}.png")):
            raise RuntimeError("Could not save isolated sweep gradient")
        y = math.floor(359 * scale)
        rows.append({"scale": scale, "device_row": y,
                     "origin_device_x": math.floor(24 * scale),
                     "alpha": [image.pixelColor(x, y).alpha() for x in range(math.floor(24 * scale), math.ceil(228 * scale))]})
    (output / "rows.json").write_text(json.dumps({"qt_platform": application.platformName(), "rows": rows}, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    render(parser.parse_args().output)
