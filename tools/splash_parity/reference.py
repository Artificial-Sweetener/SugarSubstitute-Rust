# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Render the production PySide splash with controlled clocks and blank output."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


def render_reference(legacy_root: Path, output: Path) -> None:
    """Capture actual widgets rather than a reconstructed reference drawing."""
    sys.path.insert(0, str(legacy_root))
    from PySide6.QtCore import QPoint
    from PySide6.QtGui import QColor, QFontDatabase, QImage, QPainter
    from PySide6.QtWidgets import QApplication, QWidget
    from qfluentwidgets import (  # type: ignore[import-untyped]
        Theme,
        setTheme,
        setThemeColor,
    )
    from substitute.presentation.shell.splash_progress_panel import SplashProgressPanel
    from substitute.presentation.shell.splash_window import SplashWindow
    from substitute.presentation.shell.titlebar_buttons import ComfyOutputToggleButton
    from substitute.presentation.splash_animation.paper_flip_widget import (
        SplashFlipPhase,
    )
    from sugarsubstitute_shared.launch_splash.progress import SplashProgress

    application = QApplication.instance() or QApplication([])
    # Qt's Windows offscreen backend does not discover the native Windows fonts.
    # Load the same installed family explicitly, keeping fonts out of the payload.
    if sys.platform == "win32":
        font_path = Path(os.environ["WINDIR"]) / "Fonts" / "segoeui.ttf"
        if QFontDatabase.addApplicationFont(str(font_path)) < 0:
            raise RuntimeError("The native Segoe UI reference font could not be loaded")
    output.mkdir(parents=True, exist_ok=True)
    scenarios = []
    for theme in ("dark", "light"):
        setTheme(Theme.DARK if theme == "dark" else Theme.LIGHT)
        setThemeColor("#E91E63")
        splash = SplashWindow(
            backdrop_mode=None, theme_mode=theme, activity_clock=lambda: 0.0
        )
        visual = splash._visual
        visual._timer.stop()
        visual._clock = lambda: 0.0
        visual._current_pose = visual._poses[0]
        visual._next_pose = visual._poses[1]
        panel = splash.findChild(SplashProgressPanel)
        toggle = splash.findChild(ComfyOutputToggleButton)
        assert panel is not None and toggle is not None
        splash.show()
        application.processEvents()
        for expanded in (False, True):
            panel.set_details_visible(expanded)
            application.processEvents()
            for scale in (1.0, 1.5, 2.0):
                cases = [
                    (0, 1, flip, fill, sweep)
                    for flip, fill, sweep in (
                        (0.0, 0.0, 0.0),
                        (0.0, 0.4, 0.5),
                        (0.25, 0.4, 0.5),
                        (0.5, 0.4, 0.5),
                        (0.75, 0.4, 0.5),
                        (0.0, 1.0, 0.0),
                        (0.125, 0.4, 0.5),
                        (0.375, 0.4, 0.5),
                        (0.625, 0.4, 0.5),
                        (0.875, 0.4, 0.5),
                        (0.94, 0.4, 0.5),
                    )
                ]
                cases.extend((0, -1, flip, 0.4, 0.5) for flip in (0.25, 0.75))
                if theme == "dark" and expanded and scale == 1.0:
                    cases.extend((pose, 1, 0.0, 0.4, 0.5) for pose in range(1, 40))
                for pose, direction, flip, fill, sweep in cases:
                    visual._current_pose = visual._poses[pose]
                    panel.reset_progress()
                    panel.set_progress(
                        SplashProgress(round(fill * 100), 100), status="Loading..."
                    )
                    if fill < 1:
                        panel.progress.set_activity_enabled(True)
                        panel.progress.record_activity()
                        panel.progress._sweep.pause()
                        panel.progress._sweep.setCurrentTime(round(sweep * 1800))
                        # The production renderer checks Running; resume but control its phase.
                        panel.progress._sweep.resume()
                        panel.progress._phase = sweep
                    else:
                        panel.progress.set_activity_enabled(False)
                    visual._phase = (
                        SplashFlipPhase.HOLD if flip == 0 else SplashFlipPhase.FLIP
                    )
                    visual._phase_started_at = -flip * 0.390
                    visual._direction = direction
                    width, height = round(558 * scale), round(558 * scale)
                    image = QImage(
                        width, height, QImage.Format.Format_ARGB32_Premultiplied
                    )
                    image.setDevicePixelRatio(scale)
                    image.fill(QColor(0, 0, 0, 0))
                    painter = QPainter(image)
                    splash.render(painter, QPoint())
                    painter.end()
                    name = f"{theme}-{'expanded' if expanded else 'collapsed'}-{scale:g}-{flip:g}-{fill:g}-{sweep:g}"
                    if pose != 0 or direction != 1:
                        name += f"-pose-{pose}-direction-{direction}"
                    assert image.save(str(output / f"{name}.png"))
                    scenarios.append(
                        {
                            "name": name,
                            "theme": theme,
                            "expanded": expanded,
                            "scale": scale,
                            "flip": flip,
                            "fill": fill,
                            "sweep": sweep,
                            "pose": pose,
                            "direction": direction,
                        }
                    )
                    panel.progress._sweep.stop()

            def geometry(widget: QWidget, window: QWidget = splash) -> list[int]:
                """Record resolved widget bounds within this reference window."""
                point = widget.mapTo(window, QPoint())
                return [point.x(), point.y(), widget.width(), widget.height()]

            (
                output
                / f"geometry-{theme}-{'expanded' if expanded else 'collapsed'}.json"
            ).write_text(
                json.dumps(
                    {
                        "mascot": geometry(visual),
                        "panel": geometry(panel),
                        "progress": geometry(panel.progress),
                        "status": geometry(panel.status),
                        "console": geometry(panel.details),
                        "toggle": geometry(toggle),
                        "close": geometry(splash.titleBar.closeBtn),
                        "font": panel.status.font().toString(),
                        "ascent": panel.status.fontMetrics().ascent(),
                        "descent": panel.status.fontMetrics().descent(),
                        "bar_color": panel.progress.barColor().name(),
                        "log_empty": splash.log_view.toPlainText() == "",
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
        splash.dismiss()
        splash.deleteLater()
        application.processEvents()
    (output / "scenarios.json").write_text(
        json.dumps(scenarios, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({"renders": len(scenarios), "output": str(output)}))


def main() -> None:
    """Accept explicit checkout and artifact roots without workstation defaults."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    render_reference(
        arguments.legacy_root.resolve(strict=True), arguments.output.resolve()
    )


if __name__ == "__main__":
    main()
