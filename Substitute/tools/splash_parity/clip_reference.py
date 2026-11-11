# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Observe production Qt widget clipping with a temporary opaque pose in memory."""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import replace
from pathlib import Path


def render(legacy: Path, output: Path) -> None:
    """Keep legacy files unchanged while exposing every edge of its real mascot widget."""
    sys.path.insert(0, str(legacy))
    from PySide6.QtCore import QPoint
    from PySide6.QtGui import QColor, QImage, QPainter, QPixmap
    from PySide6.QtWidgets import QApplication
    from substitute.presentation.shell.splash_window import SplashWindow
    from substitute.presentation.splash_animation.paper_flip_widget import SplashFlipPhase
    from PIL import Image

    app = QApplication.instance() or QApplication([])
    splash = SplashWindow(backdrop_mode=None, activity_clock=lambda: 0.0)
    visual = splash._visual
    visual._timer.stop()
    visual._clock = lambda: 0.0
    pixmap = QPixmap(386, 386)
    pixmap.fill(QColor(255, 255, 255, 255))
    pose = replace(visual._poses[0], pixmap=pixmap)
    visual._current_pose = visual._next_pose = pose
    splash.show()
    app.processEvents()
    output.mkdir(parents=True, exist_ok=True)
    receipts = []
    for scale in (1.25, 1.5, 1.75):
        for flip in (0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875):
            visual._phase = SplashFlipPhase.HOLD if flip == 0 else SplashFlipPhase.FLIP
            visual._phase_started_at = -flip * 0.390
            visual._direction = 1
            size = math.floor(558 * scale + 0.5)
            image = QImage(size, size, QImage.Format.Format_ARGB32_Premultiplied)
            image.setDevicePixelRatio(scale)
            image.fill(QColor(0, 0, 0, 0))
            painter = QPainter(image)
            origin = visual.mapTo(splash, QPoint())
            visual.render(painter, origin)
            painter.end()
            name = f"opaque-{scale:g}-{flip:g}"
            path = output / f"{name}.png"
            if not image.save(str(path)):
                raise RuntimeError("Could not save opaque reference")
            with Image.open(path) as exported:
                bounds = exported.getchannel("A").getbbox()
            receipts.append({"name": name, "scale": scale, "flip": flip,
                             "logical_widget": [origin.x(), origin.y(), visual.width(), visual.height()],
                             "alpha_bounds": bounds})
    (output / "clip.json").write_text(json.dumps(receipts, indent=2) + "\n", encoding="utf-8")
    splash.dismiss()
    splash.deleteLater()
    app.processEvents()


def main() -> None:
    """Require explicit reference and output roots for a controlled offscreen fixture."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    render(args.legacy_root.resolve(strict=True), args.output.resolve())


if __name__ == "__main__":
    main()
