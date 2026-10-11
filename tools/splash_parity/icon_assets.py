# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Capture production caption glyph alpha at supported device scales."""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

SCALES = (1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0)


def capture(legacy_root: Path, output: Path) -> None:
    """Render real caption buttons with their original global pixel phase."""
    sys.path.insert(0, str(legacy_root))
    from PIL import Image
    from PySide6.QtCore import QPoint
    from PySide6.QtGui import QColor, QImage, QPainter
    from PySide6.QtWidgets import QApplication, QWidget
    from qfluentwidgets import Theme, setTheme
    from qframelesswindow.titlebar.title_bar_buttons import TitleBarButtonState
    from substitute.presentation.shell.splash_window import SplashWindow

    application = QApplication.instance() or QApplication([])
    setTheme(Theme.DARK)
    splash = SplashWindow(backdrop_mode=None, theme_mode="dark")
    splash._visual._timer.stop()
    splash.show()
    application.processEvents()
    buttons = {"close": splash.titleBar.closeBtn, "console": splash._console_button}
    buttons["console"].setChecked(False)
    application.processEvents()
    output.mkdir(parents=True, exist_ok=True)
    for name, button in buttons.items():
        button.setState(TitleBarButtonState.NORMAL)
        button.setNormalColor(QColor("#ffffff"))
        button.setNormalBackgroundColor(QColor(0, 0, 0, 0))
        origin = button.mapTo(splash, QPoint())
        if (origin.x(), origin.y(), button.width(), button.height()) != (
            512 if name == "close" else 466,
            0,
            46,
            32,
        ):
            raise RuntimeError(f"Unexpected production {name} caption bounds")
        for scale in SCALES:
            image = QImage(
                round(558 * scale),
                round(32 * scale),
                QImage.Format.Format_ARGB32_Premultiplied,
            )
            image.setDevicePixelRatio(scale)
            image.fill(QColor(0, 0, 0, 0))
            painter = QPainter(image)
            button.render(painter, origin, renderFlags=QWidget.RenderFlag.DrawChildren)
            painter.end()
            raw_path = output / f"{name}-{round(scale * 100)}-reference.png"
            if not image.save(str(raw_path)):
                raise RuntimeError(f"Cannot save {name} caption reference")
            with Image.open(raw_path) as raw:
                x = math.floor(origin.x() * scale)
                crop = raw.convert("RGBA").crop(
                    (x, 0, x + math.floor(46 * scale + 0.5), round(32 * scale))
                )
                alpha = crop.getchannel("A")
                if alpha.getbbox() is None:
                    raise RuntimeError(f"Empty {name} caption glyph")
                glyph = Image.new("RGBA", crop.size, (255, 255, 255, 0))
                glyph.putalpha(alpha)
                glyph.save(output / f"{name}-{round(scale * 100)}.png")
    splash.dismiss()
    splash.deleteLater()
    application.processEvents()
    print(f"Captured {len(SCALES) * len(buttons)} production caption glyphs")


def main() -> None:
    """Require explicit checkout and bulky capture paths."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    capture(arguments.legacy_root.resolve(strict=True), arguments.output.resolve())


if __name__ == "__main__":
    main()
