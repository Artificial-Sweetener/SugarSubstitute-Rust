# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Render installed original caption widgets into hidden native Qt reference buffers."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path


def render_reference(output: Path) -> None:
    """Record exact BGRA, colors and complete-pixel digests without showing a widget."""
    os.environ["QT_QPA_PLATFORM"] = "windows"
    from PySide6 import __version__ as pyside_version
    from PySide6.QtCore import QPoint, Qt
    from PySide6.QtGui import QImage, QPainter, QRegion
    from PySide6.QtWidgets import QApplication, QWidget
    from qfluentwidgets import FluentStyleSheet, Theme
    from qframelesswindow.titlebar.title_bar_buttons import (
        CloseButton,
        MaximizeButton,
        MinimizeButton,
        TitleBarButtonState,
    )

    application = QApplication.instance() or QApplication([])
    if application.platformName() != "windows":
        raise RuntimeError("Native Windows Qt is required for caption references")
    output.mkdir(parents=True, exist_ok=True)
    records = []
    kinds = {"minimize": MinimizeButton, "maximize": MaximizeButton,
             "restore": MaximizeButton, "close": CloseButton}
    for dpi in (96, 120, 144, 168, 192, 240, 288, 336, 384):
        scale = dpi / 96
        for dark in (False, True):
            for kind, button_type in kinds.items():
                root = QWidget()
                root.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
                root.resize(46, 32)
                button = button_type(root)
                button.devicePixelRatioF = lambda: scale
                if kind == "restore":
                    button.setMaxState(True)
                FluentStyleSheet.FLUENT_WINDOW.apply(
                    button, theme=Theme.DARK if dark else Theme.LIGHT
                )
                root.ensurePolished()
                button.ensurePolished()
                for state in TitleBarButtonState:
                    button.setState(state)
                    image = QImage(
                        math.floor(46 * scale + 0.5), math.floor(32 * scale + 0.5),
                        QImage.Format.Format_ARGB32_Premultiplied,
                    )
                    image.setDevicePixelRatio(scale)
                    image.fill(Qt.GlobalColor.transparent)
                    painter = QPainter(image)
                    root.render(painter, QPoint(), QRegion(), root.RenderFlag.DrawChildren)
                    painter.end()
                    data = bytes(image.constBits())
                    digest = 0xcbf29ce484222325
                    for byte in data:
                        digest = ((digest ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
                    name = f'{dpi}-{"dark" if dark else "light"}-{kind}-{state.name.lower()}.bgra'
                    (output / name).write_bytes(data)
                    records.append({
                        "dpi": dpi, "dark": dark, "kind": kind, "state": state.name.lower(),
                        "width": image.width(), "height": image.height(),
                        "stride": image.bytesPerLine(), "file": name,
                        "sha256": hashlib.sha256(data).hexdigest(), "fnv1a64": f"{digest:016x}",
                        "native_colors": [[c.red(), c.green(), c.blue(), c.alpha()]
                                          for c in button._getColors()],
                    })
                button.close()
                root.close()
    (output / "reference.json").write_text(json.dumps({
        "schema_version": 1, "platform": application.platformName(),
        "pyside_version": pyside_version, "records": records,
    }, indent=2), encoding="utf-8")


def main() -> None:
    """Write bulky reference output only to the explicitly supplied artifact directory."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    render_reference(parser.parse_args().output)


if __name__ == "__main__":
    main()
