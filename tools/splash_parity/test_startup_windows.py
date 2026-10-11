# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Verify startup fixtures launch the production reference and preserve cache/environment state."""

import argparse
import os
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

from PIL import Image

from tools.splash_parity.startup_windows import launch, observe


class StartupLaunchTests(unittest.TestCase):
    """Inspect launch boundaries without opening native windows."""

    def setUp(self) -> None:
        """Own all temporary fixtures under the caller's configured TEMP root."""
        temporary = tempfile.TemporaryDirectory(prefix="startup-launch-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.executable = self.root / "fixture.exe"
        self.executable.write_bytes(b"fixture")

    def arguments(self, mode: str) -> argparse.Namespace:
        """Select explicit trusted entrypoints and an isolated output location."""
        return argparse.Namespace(mode=mode, temporary=self.root, legacy_root=self.root,
                                  interpreter=self.executable, executable=self.executable,
                                  duration_ms=5000, locale="ja", light=True)

    def test_production_reference_uses_existing_interpreter_without_legacy_writes(self) -> None:
        """The actual startup module remains authoritative and inherited offscreen mode is removed."""
        with patch.dict(os.environ, {"QT_QPA_PLATFORM": "offscreen", "REFERENCE_CACHE_MARKER": "retained"}):
            command, cwd, environment = launch(self.arguments("reference"), self.root)
            self.assertEqual(command[:3], [str(self.executable), "-m", "substitute.app.bootstrap.shared_splash_host"])
            self.assertEqual(cwd, self.root)
            self.assertEqual(environment["PYTHONDONTWRITEBYTECODE"], "1")
            self.assertEqual(environment["TEMP"], str(self.root))
            self.assertEqual(environment["TMP"], str(self.root))
            self.assertNotIn("QT_QPA_PLATFORM", environment)
            self.assertEqual(environment["REFERENCE_CACHE_MARKER"], "retained")
            self.assertEqual(os.environ["QT_QPA_PLATFORM"], "offscreen")

    def test_rust_and_reference_use_same_initial_collapsed_disclosure_and_locale(self) -> None:
        """The Rust fixture receives its supported presentation flags and submission receipt."""
        command, cwd, _ = launch(self.arguments("rust"), self.root)
        self.assertEqual(command[0], str(self.executable))
        self.assertIn("--collapsed", command)
        self.assertIn("--light", command)
        self.assertEqual(command[command.index("--locale") + 1], "ja")
        self.assertEqual(command[command.index("--receipt") + 1], str(self.root / "submission.json"))
        self.assertEqual(cwd, self.root)

    def test_startup_requires_real_mouse_gesture_and_causal_compositor_response(self) -> None:
        """Posted mouse messages cannot substitute for a user's actual native caption path."""
        native = Mock()
        native.find_window.return_value = 1
        native.client.return_value = Mock(width=558, height=558, dpi=96, scale=1)
        native.capture.return_value = Image.new("RGB", (558, 558), (60, 60, 60))
        process = Mock(pid=42, returncode=0)
        process.communicate.return_value = (b"", b"")
        process.poll.return_value = 0
        pointer = MagicMock()
        pointer.__enter__.return_value = pointer
        with patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.startup_windows.NativePointer", return_value=pointer), \
                patch("tools.splash_parity.startup_windows.observe_compositor",
                      return_value=(Image.new("RGB", (558, 558)), .05)), \
                patch("tools.splash_parity.startup_windows.time.perf_counter", side_effect=[0, .2]):
            receipt = observe(self.arguments("rust"), self.root / "observation", 0, native)
        pointer.move.assert_called_once_with((489, 16))
        pointer.press.assert_called_once()
        pointer.release.assert_called_once()
        native.post.assert_called_once_with(1, 42, 0x0010, 0, 0)
        self.assertEqual(receipt["startup"]["visible_frame_ms"], 50)
        self.assertEqual(receipt["startup"]["interactive_control_ms"], 200)
        self.assertTrue(receipt["startup"]["accepted"])
        saved = json.loads((self.root / "observation" / "startup.json").read_text(encoding="utf-8"))
        self.assertIn("SendInput", saved["caption_gesture"]["input_source"])


if __name__ == "__main__":
    unittest.main()
