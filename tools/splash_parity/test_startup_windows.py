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

from tools.splash_parity.startup_windows import ReferenceOwner, launch, observe, reference_base


class ReferenceOwnershipTests(unittest.TestCase):
    """Fake only ancestry and retained process handles, enforcing the production interpreter contract."""

    def setUp(self) -> None:
        """Build isolated expected executable identities without launching a native window."""
        temporary = tempfile.TemporaryDirectory(prefix="reference-process-owner-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.launcher, self.base = self.root / "launcher.exe", self.root / "base.exe"
        self.launcher.write_bytes(b"launcher")
        self.base.write_bytes(b"base")
        self.process = Mock(pid=42, _handle=7)
        self.process.poll.return_value = None
        self.processes = Mock()
        self.identities = {7: (42, self.launcher, 100), 9: (83, self.base, 110), 10: (84, self.base, 120)}
        self.processes.identity.side_effect = lambda handle: self.identities[handle]
        self.processes.children.return_value = [(11, "conhost.exe"), (83, "base.exe")]
        self.processes.open.return_value = 9
        self.processes.alive.return_value = True

    def owner(self) -> ReferenceOwner:
        """Borrow the original Popen native handle and verify only direct base-interpreter children."""
        return ReferenceOwner(self.process, self.launcher, self.base, self.processes)

    def test_exact_direct_interpreter_child_is_owned_and_conhost_excluded(self) -> None:
        """The Qt PID may differ from the redirector PID without adopting unrelated descendants."""
        owner = self.owner()
        owner.bind(float("inf"))
        self.assertEqual(owner.pid, 83)
        self.assertEqual(owner.metadata["launcher_pid"], 42)
        self.processes.open.assert_called_once_with(83)
        self.assertIsNone(owner.poll())
        owner.close()
        self.processes.stop.assert_called_once_with(9)
        self.processes.close.assert_called_once_with(9)

    def test_foreign_path_old_creation_and_ambiguous_children_reject(self) -> None:
        """Matching filenames and parent PIDs are insufficient to admit a reference window owner."""
        foreign = self.root / "foreign" / "base.exe"
        foreign.parent.mkdir()
        foreign.write_bytes(b"foreign")
        for identity in ((83, foreign, 110), (83, self.base, 90)):
            with self.subTest(identity=identity):
                self.identities[9] = identity
                owner = self.owner()
                with self.assertRaises(RuntimeError):
                    owner.bind(float("inf"))
                self.assertIsNone(owner.handle)
        self.identities[9] = (83, self.base, 110)
        self.processes.children.return_value = [(83, "base.exe"), (84, "base.exe")]
        self.processes.open.side_effect = [9, 10]
        with self.assertRaisesRegex(RuntimeError, "multiple matching"):
            self.owner().bind(float("inf"))
        self.assertTrue(any(call.args == (10,) for call in self.processes.close.call_args_list))
        self.processes.stop.assert_not_called()

    def test_missing_child_and_dead_launcher_fail_with_original_deadline(self) -> None:
        """A conhost-only tree or terminated redirector cannot produce an accepted window owner."""
        self.processes.children.return_value = [(11, "conhost.exe")]
        with patch("tools.splash_parity.startup_windows.time.perf_counter", side_effect=[0, 2]), \
                patch("tools.splash_parity.startup_windows.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "launch deadline"):
                self.owner().bind(1)
        self.processes.open.assert_not_called()
        self.process.poll.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "launch deadline"):
            self.owner().bind(float("inf"))

    def test_original_launcher_handle_identity_and_child_terminal_state_are_required(self) -> None:
        """Native retained handles prevent borrowing an unrelated or reused PID's process."""
        self.identities[7] = (43, self.launcher, 100)
        with self.assertRaisesRegex(RuntimeError, "launcher native handle"):
            self.owner()
        self.identities[7] = (42, self.launcher, 100)
        owner = self.owner()
        owner.bind(float("inf"))
        self.processes.alive.return_value = False
        self.assertEqual(owner.poll(), 0)

    def test_failed_stop_still_closes_child_handle_and_preserves_cleanup_cause(self) -> None:
        """Failed terminal observation remains reviewable even after native handle release."""
        owner = self.owner()
        owner.bind(float("inf"))
        self.processes.stop.side_effect = RuntimeError("terminal timeout")
        with self.assertRaises(ExceptionGroup) as caught:
            owner.close()
        self.assertIn("terminal timeout", str(caught.exception.exceptions[0]))
        self.processes.close.assert_called_once_with(9)

    def test_base_probe_requires_bounded_absolute_existing_executable_metadata(self) -> None:
        """Resolve the trusted venv base before the startup timer without accepting relative paths."""
        with patch("tools.splash_parity.startup_windows.subprocess.run",
                   return_value=Mock(returncode=0, stdout=json.dumps({"base_executable": str(self.base)}).encode())) as probe:
            self.assertEqual(reference_base(self.launcher, {}), self.base)
        self.assertEqual(probe.call_args.kwargs["timeout"], 5)
        with patch("tools.splash_parity.startup_windows.subprocess.run",
                   return_value=Mock(returncode=0, stdout=b'{"base_executable":"relative.exe"}')):
            with self.assertRaisesRegex(RuntimeError, "absolute"):
                reference_base(self.launcher, {})


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
        fixture = MagicMock(receipt={"fixture": "actual click"})
        with patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.NativePointer", return_value=pointer), \
                patch("tools.splash_parity.startup_windows.observe_compositor",
                      return_value=(Image.new("RGB", (558, 558)), .05, .04)), \
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
        self.assertEqual(saved["creation_flags"], 0x08000000)
        fixture.__enter__.assert_called_once()
        fixture.allow_product.assert_called_once_with(42)
        fixture.product_exposed.assert_called_once()
        fixture.__exit__.assert_called_once()

    def test_failed_launch_activation_never_starts_measured_product(self) -> None:
        """An inactive launch setup remains a preparation failure, without a forged startup result."""
        fixture = MagicMock(receipt={"last_physical_hit_probe": {"hit_hwnd": 9}})
        fixture.__enter__.side_effect = RuntimeError("activation rejected")
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen") as launch_process:
            with self.assertRaisesRegex(RuntimeError, "activation rejected"):
                observe(self.arguments("rust"), self.root / "activation-failed", 0, Mock())
        launch_process.assert_not_called()
        receipt = json.loads((self.root / "activation-failed" / "failure.json").read_text())
        self.assertEqual(receipt["failure_phase"], "before measured product launch")
        self.assertEqual(receipt["foreground_launch_setup"]["last_physical_hit_probe"]["hit_hwnd"], 9)

    def test_process_creation_failure_still_destroys_owned_launcher(self) -> None:
        """A failed Popen cannot leak the native setup fixture."""
        fixture = MagicMock(receipt={})
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen", side_effect=OSError("launch rejected")):
            with self.assertRaisesRegex(OSError, "launch rejected"):
                observe(self.arguments("rust"), self.root / "popen-failed", 0, Mock())
        fixture.__exit__.assert_called_once()


if __name__ == "__main__":
    unittest.main()
