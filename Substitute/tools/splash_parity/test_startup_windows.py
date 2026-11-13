# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Verify startup fixtures launch the production reference and preserve cache/environment state."""

import argparse
import ctypes
import os
import json
import tempfile
import unittest
from pathlib import Path
from ctypes import wintypes
from unittest.mock import MagicMock, Mock, patch

from PIL import Image

from tools.splash_parity.startup_windows import ReferenceOwner, ReferenceProcesses, launch, observe, reference_base, retain_diagnostics


class RetainedProcessIdentityTests(unittest.TestCase):
    """Exercise actual retained-object identity APIs with fake native calls, including after exit."""

    def native(self):
        native = object.__new__(ReferenceProcesses)
        native.kernel = Mock()
        native.kernel.GetProcessId.return_value = 42
        def timestamps(handle, creation, *remaining):
            self.assertEqual(handle, 7)
            value = ctypes.cast(creation, ctypes.POINTER(wintypes.FILETIME)).contents
            value.dwHighDateTime, value.dwLowDateTime = 3, 19
            return True
        native.kernel.GetProcessTimes.side_effect = timestamps
        return native

    def test_exit_wait_requires_real_signal_on_original_handle(self):
        native = self.native()
        for status, exited in ((0, True), (0x102, False)):
            native.kernel.WaitForSingleObject.return_value = status
            self.assertEqual(native.wait_exited(7, 100), exited)
            native.kernel.WaitForSingleObject.assert_called_with(7, 100)
        native.kernel.WaitForSingleObject.return_value = 0xFFFFFFFF
        with self.assertRaisesRegex(RuntimeError, "wait status"):
            native.wait_exited(7, 100)

    def test_exit_wait_rejects_unbounded_or_malformed_timeouts_before_native_call(self):
        native = self.native()
        for timeout in (-1, 2001, 0xFFFFFFFF, True, 100.0, None):
            with self.subTest(timeout=timeout), self.assertRaises(ValueError):
                native.wait_exited(7, timeout)
        native.kernel.WaitForSingleObject.assert_not_called()

    def test_retained_lifetime_reads_pid_and_creation_without_live_image_query(self):
        native = self.native()
        native.kernel.QueryFullProcessImageNameW.side_effect = OSError("image query unavailable after exit")
        self.assertEqual(native.lifetime_identity(7), (42, (3 << 32) | 19))
        native.kernel.GetProcessId.assert_called_once_with(7)
        native.kernel.GetProcessTimes.assert_called_once()
        native.kernel.QueryFullProcessImageNameW.assert_not_called()

    def test_invalid_handles_and_scalar_metadata_do_not_establish_identity(self):
        for handle in (0, -1, True, 7.0, None, ctypes.c_void_p(-1).value):
            native = self.native()
            with self.subTest(handle=handle), self.assertRaises(ValueError):
                native.lifetime_identity(handle)
            self.assertEqual(native.kernel.mock_calls, [])
        for pid in (True, 42.0, -1, 0x100000000):
            native = self.native()
            native.kernel.GetProcessId.return_value = pid
            with self.subTest(pid=pid), self.assertRaises(RuntimeError):
                native.lifetime_identity(7)
            native.kernel.GetProcessTimes.assert_not_called()
        native = self.native()
        native.kernel.GetProcessTimes.side_effect = None
        native.kernel.GetProcessTimes.return_value = True
        self.assertRaisesRegex(RuntimeError, "creation time", native.lifetime_identity, 7)

    def test_failed_native_pid_or_times_preserves_original_cause(self):
        for operation in ("GetProcessId", "GetProcessTimes"):
            native = self.native()
            call = getattr(native.kernel, operation)
            call.side_effect, call.return_value = None, 0
            cause = OSError(f"native {operation} failed")
            with self.subTest(operation=operation), patch("tools.splash_parity.startup_windows.ctypes.WinError", return_value=cause):
                with self.assertRaises(OSError) as caught:
                    native.lifetime_identity(7)
            self.assertIs(caught.exception, cause)
            native.kernel.QueryFullProcessImageNameW.assert_not_called()

    def test_live_path_identity_reuses_lifetime_and_keeps_path_failure(self):
        native = self.native()
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "live.exe"
            executable.write_bytes(b"fixture")
            def image(handle, flags, target, capacity):
                self.assertEqual((handle, flags), (7, 0))
                target.value = str(executable)
                return True
            native.kernel.QueryFullProcessImageNameW.side_effect = image
            self.assertEqual(native.identity(7), (42, executable.resolve(strict=True), (3 << 32) | 19))
            native.kernel.GetProcessId.assert_called_once_with(7)
            native.kernel.GetProcessTimes.assert_called_once()
            native.kernel.QueryFullProcessImageNameW.side_effect = None
            native.kernel.QueryFullProcessImageNameW.return_value = False
            cause = OSError("live image query failed")
            with patch("tools.splash_parity.startup_windows.ctypes.WinError", return_value=cause):
                with self.assertRaises(OSError) as caught:
                    native.identity(7)
            self.assertIs(caught.exception, cause)


class ProcessSuspensionTests(unittest.TestCase):
    """Fault injection adds suspend permission only after matching retained native identity."""

    def setUp(self):
        self.processes = object.__new__(ReferenceProcesses)
        self.processes.kernel = Mock()
        self.processes.native = Mock()
        self.processes.kernel.OpenProcess.return_value = 9
        self.processes.kernel.CloseHandle.return_value = True
        self.processes.native.NtSuspendProcess.return_value = 0
        self.processes.lifetime_identity = Mock(side_effect=[(42, 100), (42, 100)])

    def test_suspend_uses_verified_permission_handle_and_releases_only_that_handle(self):
        self.processes.suspend(7)
        self.processes.kernel.OpenProcess.assert_called_once_with(0x1800, False, 42)
        self.assertEqual(self.processes.lifetime_identity.call_args_list,
                         [unittest.mock.call(7), unittest.mock.call(9)])
        self.processes.native.NtSuspendProcess.assert_called_once_with(9)
        self.processes.kernel.CloseHandle.assert_called_once_with(9)
        self.processes.kernel.TerminateProcess.assert_not_called()

    def test_changed_identity_never_receives_suspend_and_new_handle_is_closed(self):
        self.processes.lifetime_identity.side_effect = [(42, 100), (42, 101)]
        with self.assertRaisesRegex(RuntimeError, "differs from retained custody"):
            self.processes.suspend(7)
        self.processes.native.NtSuspendProcess.assert_not_called()
        self.processes.kernel.CloseHandle.assert_called_once_with(9)

    def test_native_suspend_failure_remains_observable_and_releases_new_handle(self):
        self.processes.native.NtSuspendProcess.return_value = -0x3FFFFFDE
        with self.assertRaisesRegex(RuntimeError, "-0x3fffffde"):
            self.processes.suspend(7)
        self.processes.kernel.CloseHandle.assert_called_once_with(9)

    def test_permission_failure_does_not_fault_or_close_original_process(self):
        self.processes.kernel.OpenProcess.return_value = 0
        cause = OSError("Suspend permission denied")
        with patch("tools.splash_parity.startup_windows.ctypes.WinError", return_value=cause):
            with self.assertRaises(OSError) as caught:
                self.processes.suspend(7)
        self.assertIs(caught.exception, cause)
        self.processes.native.NtSuspendProcess.assert_not_called()
        self.processes.kernel.CloseHandle.assert_not_called()

    def test_unverifiable_original_never_opens_or_suspends_a_process(self):
        self.processes.lifetime_identity.side_effect = OSError("Identity unavailable")
        with self.assertRaises(OSError):
            self.processes.suspend(7)
        self.processes.kernel.OpenProcess.assert_not_called()
        self.processes.native.NtSuspendProcess.assert_not_called()


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
        self.processes.ancestry.return_value = {42: [(11, "conhost.exe"), (83, "base.exe")]}
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
        self.processes.ancestry.return_value = {42: [(83, "base.exe"), (84, "base.exe")]}
        self.processes.open.side_effect = [9, 10]
        with self.assertRaisesRegex(RuntimeError, "multiple matching"):
            self.owner().bind(float("inf"))
        self.assertTrue(any(call.args == (10,) for call in self.processes.close.call_args_list))
        self.processes.stop.assert_not_called()

    def test_missing_child_and_dead_launcher_fail_with_original_deadline(self) -> None:
        """A conhost-only tree or terminated redirector cannot produce an accepted window owner."""
        self.processes.ancestry.return_value = {42: [(11, "conhost.exe")]}
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

    def arguments(self, mode: str, profile="strict") -> argparse.Namespace:
        """Select explicit trusted entrypoints and an isolated output location."""
        return argparse.Namespace(mode=mode, temporary=self.root, legacy_root=self.root,
                                  interpreter=self.executable, executable=self.executable,
                                  duration_ms=5000, locale="ja", light=True, timing_audit=False, timing_profile=profile)

    def test_unknown_profile_rejects_before_output_or_launch_effects(self):
        target = self.root / "invalid-profile"
        with patch("tools.splash_parity.startup_windows.subprocess.Popen") as process:
            self.assertRaises(ValueError, observe, self.arguments("rust", "unknown"), target, 0, Mock())
        self.assertFalse(target.exists())
        process.assert_not_called()

    def test_busy_startup_uses_actual_observations_and_same_physical_gesture(self):
        native = Mock()
        native.find_window.return_value = 1
        native.client.return_value = Mock(width=558, height=558, dpi=96, scale=1)
        native.capture.return_value = Image.new("RGB", (558, 558), (60, 60, 60))
        process = Mock(pid=42, returncode=0)
        process.communicate.return_value = (b"", b"")
        process.poll.return_value = 0
        pointer, fixture = MagicMock(), MagicMock(receipt={})
        pointer.__enter__.return_value = pointer
        with patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.NativePointer", return_value=pointer), \
                patch("tools.splash_parity.startup_windows.observe_compositor", return_value=(Image.new("RGB", (558, 558)), .6, .04)), \
                patch("tools.splash_parity.startup_windows.time.perf_counter", side_effect=[0, .8]):
            result = observe(self.arguments("rust", "busy"), self.root / "busy-observation", 0, native)
        self.assertTrue(result["startup"]["accepted"])
        self.assertFalse(result["startup"]["timing_assessment"]["strict_accepted"])
        self.assertEqual(result["startup"]["interactive_control_ms"], 800)
        pointer.move.assert_called_once_with((489, 16))
        pointer.press.assert_called_once()
        pointer.release.assert_called_once()

    def test_timing_diagnostics_preserve_launch_options_and_reject_reference(self) -> None:
        arguments = self.arguments("rust")
        arguments.timing_audit = True
        command, cwd, _ = launch(arguments, self.root)
        self.assertEqual(command.count("--timing-audit"), 1)
        self.assertIn("--collapsed", command)
        self.assertEqual(cwd, self.root)
        arguments.mode = "reference"
        self.assertRaises(ValueError, launch, arguments, self.root)

    def test_diagnostic_output_has_bounded_exact_tails_and_never_saves_reference(self) -> None:
        arguments = self.arguments("rust")
        arguments.timing_audit = True
        raw = b"prefix" + b"x" * 65536
        result = retain_diagnostics(arguments, self.root, b"", raw)
        self.assertEqual((self.root / "stderr-tail.log").read_bytes(), raw[-65536:])
        self.assertEqual(result["stderr"]["bytes_observed"], len(raw))
        self.assertEqual(result["stderr"]["retained_tail_bytes"], 65536)
        self.assertTrue(result["stderr"]["tail_truncated"])
        self.assertFalse(result["stdout"]["tail_truncated"])
        for mode, enabled in (("reference", True), ("rust", False)):
            arguments.mode, arguments.timing_audit = mode, enabled
            forbidden = self.root / "must-remain-absent"
            self.assertRaises(ValueError, retain_diagnostics, arguments, forbidden, b"private", b"private")
            self.assertFalse(forbidden.exists())

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

    def test_failed_diagnostic_launch_retains_bounded_owned_rust_output(self) -> None:
        arguments = self.arguments("rust")
        arguments.timing_audit = True
        native = Mock()
        native.find_window.side_effect = RuntimeError("window missing")
        process = Mock(pid=42, returncode=1)
        process.poll.return_value = 1
        process.communicate.return_value = (b"", b"native phase failure")
        fixture = MagicMock(receipt={})
        output = self.root / "diagnostic-failed"
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process):
            with self.assertRaisesRegex(RuntimeError, "window missing"):
                observe(arguments, output, 0, native)
        receipt = json.loads((output / "failure.json").read_text())
        self.assertEqual(receipt["error"], "window missing")
        self.assertEqual(receipt["diagnostic_output"]["stderr"]["bytes_observed"], 20)
        self.assertEqual((output / "stderr-tail.log").read_bytes(), b"native phase failure")
        fixture.__exit__.assert_called_once()

    def test_diagnostic_cleanup_error_retains_the_original_observation_cause(self) -> None:
        arguments = self.arguments("rust")
        arguments.timing_audit = True
        native = Mock()
        native.find_window.side_effect = RuntimeError("window missing")
        process = Mock(pid=42, returncode=1)
        process.poll.return_value = 1
        process.communicate.return_value = (b"", b"")
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=MagicMock(receipt={})), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.startup_windows.retain_diagnostics", side_effect=OSError("tail failure")):
            with self.assertRaises(ExceptionGroup) as caught:
                observe(arguments, self.root / "diagnostic-cleanup-failed", 0, native)
        self.assertEqual([str(cause) for cause in caught.exception.exceptions], ["window missing", "tail failure"])

    def test_failure_receipt_storage_never_masks_observation_or_cleanup_causes(self) -> None:
        native = Mock()
        native.find_window.side_effect = RuntimeError("window missing")
        process = Mock(pid=42, returncode=1)
        process.poll.return_value = 1
        fixture = MagicMock(receipt={})
        fixture.__exit__.side_effect = OSError("fixture cleanup failed")
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.startup_windows.write_json", side_effect=OSError("receipt storage failed")):
            with self.assertRaises(ExceptionGroup) as caught:
                observe(self.arguments("rust"), self.root / "storage-failed", 0, native)
        self.assertEqual([str(cause) for cause in caught.exception.exceptions],
                         ["window missing", "fixture cleanup failed", "receipt storage failed"])

    def test_prelaunch_failure_receipt_storage_preserves_activation_cause(self) -> None:
        fixture = MagicMock(receipt={})
        fixture.__enter__.side_effect = RuntimeError("activation rejected")
        with patch("tools.splash_parity.startup_windows.ForegroundLauncher", return_value=fixture), \
                patch("tools.splash_parity.startup_windows.write_json", side_effect=OSError("receipt storage failed")), \
                patch("tools.splash_parity.startup_windows.subprocess.Popen") as process:
            with self.assertRaises(ExceptionGroup) as caught:
                observe(self.arguments("rust"), self.root / "prelaunch-storage-failed", 0, Mock())
        self.assertEqual([str(cause) for cause in caught.exception.exceptions],
                         ["activation rejected", "receipt storage failed"])
        process.assert_not_called()


if __name__ == "__main__":
    unittest.main()
