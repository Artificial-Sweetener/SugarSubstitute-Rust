# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Render the production PySide splash with controlled clocks and blank output."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import sys
from pathlib import Path


def render_reference(legacy_root: Path, output: Path, locale: str = "en", locale_sample: bool = False) -> None:
    """Capture actual widgets rather than a reconstructed reference drawing."""
    if sys.platform == "win32":
        os.environ["QT_QPA_PLATFORM"] = "windows"
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    sys.path.insert(0, str(legacy_root))
    from PySide6.QtCore import QPoint, Qt
    from PySide6.QtGui import QColor, QFontDatabase, QImage, QPainter, QTextLayout
    from PySide6 import __version__ as pyside_version
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
    from qframelesswindow.titlebar.title_bar_buttons import TitleBarButtonState
    from substitute.app.bootstrap.splash_localization import build_splash_localization_runtime
    from sugarsubstitute_shared.presentation.localization import app_text, render_application_text

    application = QApplication.instance() or QApplication([])
    localization = build_splash_localization_runtime(application, locale_override=locale)
    locale = localization.initial_snapshot.effective_language_identifier
    output.mkdir(parents=True, exist_ok=True)
    write_environment = {
        "interpreter": sys.executable,
        "pyside_version": pyside_version,
        "qt_platform": application.platformName(),
        "font_families": QFontDatabase.families(),
        "screen_mapping": "Suppressed with WA_DontShowOnScreen; production show/resize lifecycle retained",
        "source_sha256": {
            name: hashlib.sha256((legacy_root / name).read_bytes()).hexdigest()
            for name in (
                "substitute/presentation/shell/splash_window.py",
                "substitute/presentation/shell/splash_progress_panel.py",
                "substitute/presentation/splash_animation/paper_flip_widget.py",
            )
        },
    }
    (output / "environment.json").write_text(json.dumps(write_environment, indent=2) + "\n", encoding="utf-8")
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
        # Keep the production show/resize lifecycle while Qt suppresses screen mapping.
        splash.setAttribute(Qt.WidgetAttribute.WA_DontShowOnScreen, True)
        splash.show()
        application.processEvents()
        for expanded in ((False,) if locale_sample else (False, True)):
            panel.set_details_visible(expanded)
            application.processEvents()
            for scale in (1.0, 1.25, 1.5, 1.75, 2.0):
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
                if locale_sample:
                    cases = [(0, 1, 0.0, 0.4, 0.5)]
                control_cases = [(case, None, None) for case in cases]
                control_cases.extend(
                    ((0, 1, 0.0, 0.4, 0.5), control, control if pressed else None)
                    for control in (() if locale_sample else ("console", "close"))
                    for pressed in (False, True)
                )
                for (pose, direction, flip, fill, sweep), hovered, pressed in control_cases:
                    for control, button in (("console", toggle), ("close", splash.titleBar.closeBtn)):
                        button.setAttribute(Qt.WidgetAttribute.WA_UnderMouse, False)
                        button.setState(
                            TitleBarButtonState.PRESSED if pressed == control
                            else TitleBarButtonState.HOVER if hovered == control
                            else TitleBarButtonState.NORMAL
                        )
                    visual._current_pose = visual._poses[pose]
                    panel.reset_progress()
                    panel.set_progress(
                        SplashProgress(round(fill * 100), 100),
                        status=render_application_text(app_text("Loading..."))
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
                    width = height = math.floor(558 * scale + 0.5)
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
                    if hovered:
                        name += f"-hover-{hovered}"
                    if pressed:
                        name += f"-press-{pressed}"
                    if locale != "en":
                        name += f"-locale-{locale}"
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
                            "hovered": hovered,
                            "pressed": pressed,
                            "locale": locale,
                        }
                    )
                    panel.progress._sweep.stop()

            def geometry(widget: QWidget, window: QWidget = splash) -> list[int]:
                """Record resolved widget bounds within this reference window."""
                point = widget.mapTo(window, QPoint())
                return [point.x(), point.y(), widget.width(), widget.height()]

            layout = QTextLayout(panel.status.text(), panel.status.font())
            layout.beginLayout()
            line = layout.createLine()
            line.setLineWidth(panel.status.width())
            layout.endLayout()
            glyph_runs = [
                {"font_family": run.rawFont().familyName(),
                 "font_style": run.rawFont().styleName(),
                 "glyph_indices": list(run.glyphIndexes()),
                 "positions": [[point.x(), point.y()] for point in run.positions()]}
                for run in layout.glyphRuns()
            ]
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
                        "font_families": panel.status.font().families(),
                        "ascent": panel.status.fontMetrics().ascent(),
                        "descent": panel.status.fontMetrics().descent(),
                        "bar_color": panel.progress.barColor().name(),
                        "log_empty": splash.log_view.toPlainText() == "",
                        "locale": locale,
                        "qt_platform": application.platformName(),
                        "status_text": panel.status.text(),
                        "toggle_accessible_name": toggle.accessibleName(),
                        "close_tooltip": splash.titleBar.closeBtn.toolTip(),
                        "status_glyph_runs": glyph_runs,
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
    localization.manager.close()


def main() -> None:
    """Accept explicit checkout and artifact roots without workstation defaults."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--locale", default="en", choices=("en", "zh-Hans", "ja", "ko", "es"))
    parser.add_argument("--locale-sample", action="store_true", help="Capture collapsed owned text at five scales in both themes")
    arguments = parser.parse_args()
    render_reference(
        arguments.legacy_root.resolve(strict=True), arguments.output.resolve(),
        arguments.locale, arguments.locale_sample
    )


if __name__ == "__main__":
    main()
