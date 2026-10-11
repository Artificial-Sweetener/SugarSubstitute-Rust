# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Prove native qualification observes compositor paint and causal caption feedback."""

import argparse
import ctypes
import json
import tempfile
import unittest
from pathlib import Path
from ctypes import wintypes
from unittest.mock import Mock, patch

from PIL import Image

from tools.splash_parity.native_windows import ExposureFailure, MouseEvent, NativePointer, NativeWindows, observe_caption, observe_compositor, observe_exposure, qualify


class NativeExposureMetadataTests(unittest.TestCase):
    """Keep actual observer decisions while faking window inventory and native rectangles."""

    def native(self) -> NativeWindows:
        """Provide a foreign foreground and one overlapping window above the owned client."""
        native = object.__new__(NativeWindows)
        native.user = Mock()
        native.client = Mock(return_value=Mock(width=558, height=558))
        native.user.GetForegroundWindow.return_value = 9
        native.user.GetAncestor.return_value = 9
        native.user.IsWindowVisible.return_value = True
        native.user.IsIconic.return_value = False
        native.user.GetWindow.side_effect = lambda hwnd, relation: {1: 2, 2: 3, 3: None}[hwnd]

        def origin(hwnd, pointer):
            point = ctypes.cast(pointer, ctypes.POINTER(wintypes.POINT)).contents
            point.x, point.y = 100, 200
            return True

        def rectangle(hwnd, pointer):
            rect = ctypes.cast(pointer, ctypes.POINTER(wintypes.RECT)).contents
            rect.left, rect.top, rect.right, rect.bottom = (50, 150, 400, 500) if hwnd == 2 else (1000, 0, 1500, 100)
            return True

        def identity(hwnd, pointer):
            ctypes.cast(pointer, ctypes.POINTER(wintypes.DWORD)).contents.value = 99 if hwnd == 2 else 777
            return 1

        native.user.ClientToScreen.side_effect = origin
        native.user.GetWindowRect.side_effect = rectangle
        native.user.GetWindowThreadProcessId.side_effect = identity
        return native

    def test_foreground_and_occluder_identity_explain_failed_exposure(self) -> None:
        """Keep the blocking HWND/PID/bounds without reading another application's text."""
        state = self.native().exposure_state(1, 42)
        self.assertFalse(state["foreground_owned"])
        self.assertEqual(state["foreground_pid"], 777)
        self.assertEqual(state["occluding_windows"], [{"hwnd": 2, "pid": 99, "screen_bounds": [50, 150, 400, 500]}])

    def test_unknown_visible_window_bounds_are_not_unoccluded_acceptance(self) -> None:
        """A failed external bounds query cannot conceal an overlapping native window."""
        native = self.native()
        native.user.GetWindowRect.side_effect = None
        native.user.GetWindowRect.return_value = False
        with patch("tools.splash_parity.native_windows.ctypes.WinError", return_value=OSError("unknown occluder bounds")):
            with self.assertRaisesRegex(OSError, "unknown occluder bounds"):
                native.occluded(1, 42)


class NativePointerTests(unittest.TestCase):
    """Reject foreign input and preserve human movement while releasing only injected button state."""

    def pointer(self) -> NativePointer:
        """Provide explicit external mouse and ownership boundaries."""
        pointer = object.__new__(NativePointer)
        pointer.native = Mock()
        pointer.hwnd, pointer.pid = 1, 42
        pointer.original, pointer.last, pointer.pressed = (10, 20), (30, 40), False
        pointer.cursor = Mock(return_value=(30, 40))
        pointer.native.user.GetAsyncKeyState.return_value = 0
        pointer.native.user.WindowFromPoint.return_value = 1
        pointer.native.user.GetAncestor.return_value = 1
        pointer.native.user.GetForegroundWindow.return_value = 1
        pointer.send = Mock()
        return pointer

    def test_button_only_click_batches_exact_native_array_and_records_insertions(self) -> None:
        """One native array inserts only down/up; coordinates and all movement flags stay zero."""
        import ctypes
        pointer = self.pointer()
        events = []
        def insert(count, array, size):
            packets = ctypes.cast(array, ctypes.POINTER(MouseEvent))
            events.append((count, size, [(packets[i].kind, packets[i].payload.mouse.x,
                            packets[i].payload.mouse.y, packets[i].payload.mouse.flags,
                            packets[i].payload.mouse.data, packets[i].payload.mouse.time,
                            packets[i].payload.mouse.extra) for i in range(count)]))
            return 2
        pointer.native.user.SendInput.side_effect = insert
        result = pointer.click()
        self.assertEqual(events, [(2, ctypes.sizeof(MouseEvent), [(0, 0, 0, 2, 0, 0, 0), (0, 0, 0, 4, 0, 0, 0)])])
        self.assertEqual(result["inserted_events"], 2)
        self.assertEqual(result["event_flags"], [2, 4])
        self.assertTrue(result["prechecked"] and result["postchecked"])
        self.assertEqual([call.args[0] for call in pointer.native.user.GetAsyncKeyState.call_args_list], [1, 2, 4, 5, 6] * 2)
        self.assertFalse(pointer.pressed)
        pointer.send.assert_not_called()

    def test_click_foreign_ownership_existing_press_and_all_human_buttons_reject(self) -> None:
        """Every admission boundary rejects before native insertion and preserves human state."""
        for key in (1, 2, 4, 5, 6):
            pointer = self.pointer()
            pointer.native.user.GetAsyncKeyState.side_effect = lambda observed, key=key: 0x8000 if observed == key else 0
            with self.assertRaisesRegex(RuntimeError, "already held"):
                pointer.click()
            pointer.native.user.SendInput.assert_not_called()
            pointer.__exit__(None, None, None)
            pointer.send.assert_not_called()
        pointer = self.pointer()
        pointer.native.user.GetAncestor.return_value = 2
        self.assertRaises(RuntimeError, pointer.click)
        pointer.native.user.SendInput.assert_not_called()
        pointer = self.pointer()
        pointer.pressed = True
        self.assertRaisesRegex(RuntimeError, "owned mouse press", pointer.click)
        pointer.native.user.SendInput.assert_not_called()

    def test_click_zero_partial_unknown_and_exception_keep_correct_cleanup_obligation(self) -> None:
        """Partial or uncertain delivery releases a possible owned down, never qualifies a click."""
        for inserted in (0, 1, 3, True, False, 0.0, 2.0, None, "2", RuntimeError("native insertion cause")):
            pointer = self.pointer()
            if isinstance(inserted, Exception):
                pointer.native.user.SendInput.side_effect = inserted
            else:
                pointer.native.user.SendInput.return_value = inserted
            with self.assertRaises(RuntimeError) as caught:
                pointer.click()
            self.assertEqual(pointer.pressed, type(inserted) is not int or inserted != 0)
            if not isinstance(inserted, Exception):
                self.assertEqual(pointer.last_click["inserted_events"], inserted)
            pointer.__exit__(RuntimeError, caught.exception, None)
            self.assertFalse(pointer.pressed)
            expected_release = type(inserted) is not int or inserted != 0
            self.assertEqual(any(call.args[1] == 4 for call in pointer.send.call_args_list), expected_release)

    def test_click_post_human_button_or_cursor_change_preserves_human_pointer(self) -> None:
        """Successful insertion cannot authorize restoration over an intervening human gesture."""
        for offset in range(5):
            pointer = self.pointer()
            pointer.native.user.SendInput.return_value = 2
            pointer.native.user.GetAsyncKeyState.side_effect = [0] * (5 + offset) + [0x8000]
            with self.assertRaisesRegex(RuntimeError, "human state is preserved") as caught:
                pointer.click()
            pointer.__exit__(RuntimeError, caught.exception, None)
            pointer.send.assert_not_called()
        pointer = self.pointer()
        pointer.native.user.SendInput.return_value = 2
        pointer.cursor.side_effect = [(30, 40), (30, 40), (100, 200), (100, 200)]
        with self.assertRaisesRegex(RuntimeError, "human position is preserved") as caught:
            pointer.click()
        with self.assertRaises(ExceptionGroup):
            pointer.__exit__(RuntimeError, caught.exception, None)
        pointer.send.assert_not_called()

    def test_human_button_after_successful_click_blocks_later_cursor_restoration(self) -> None:
        """A human gesture during feedback observation remains protected at context teardown."""
        pointer = self.pointer()
        pointer.native.user.SendInput.return_value = 2
        pointer.click()
        pointer.native.user.GetAsyncKeyState.side_effect = lambda key: 0x8000 if key == 2 else 0
        with self.assertRaises(ExceptionGroup) as caught:
            pointer.__exit__(None, None, None)
        self.assertIn("human state is preserved", str(caught.exception.exceptions[0]))
        pointer.send.assert_not_called()

    def test_click_failed_insertion_cannot_restore_cursor_over_new_human_button(self) -> None:
        """Zero/partial/uncertain insertion never restores the cursor after an unobserved human gesture."""
        for inserted in (0, 1, None, RuntimeError("insertion boundary failure")):
            pointer = self.pointer()
            def insert(*_, inserted=inserted):
                pointer.native.user.GetAsyncKeyState.side_effect = lambda key: 0x8000 if key == 2 else 0
                if isinstance(inserted, Exception):
                    raise inserted
                return inserted
            pointer.native.user.SendInput.side_effect = insert
            with self.assertRaises(RuntimeError) as caught:
                pointer.click()
            pointer.__exit__(RuntimeError, caught.exception, None)
            self.assertFalse(any(call.args[1] == 0 for call in pointer.send.call_args_list))
            self.assertEqual(any(call.args[1] == 4 for call in pointer.send.call_args_list), inserted != 0)

    def test_click_post_foreign_ownership_and_cleanup_secondary_cause_reject(self) -> None:
        """A post-insertion ownership loss and failed release retain original observable failures."""
        pointer = self.pointer()
        pointer.native.user.SendInput.return_value = 2
        pointer.native.user.GetAncestor.side_effect = [1, 1, 2]
        self.assertRaises(RuntimeError, pointer.click)
        self.assertFalse(pointer.pressed)
        pointer = self.pointer()
        pointer.native.user.SendInput.return_value = 1
        with self.assertRaises(RuntimeError) as caught:
            pointer.click()
        pointer.send.side_effect = RuntimeError("cleanup release cause")
        with self.assertRaises(ExceptionGroup) as grouped:
            pointer.__exit__(RuntimeError, caught.exception, None)
        self.assertIs(grouped.exception.exceptions[0], caught.exception)
        self.assertIn("cleanup release cause", str(grouped.exception.exceptions[1]))

    def test_existing_human_press_rejects_before_injection(self) -> None:
        """Qualification does not take over an existing mouse gesture."""
        pointer = self.pointer()
        pointer.native.user.GetAsyncKeyState.return_value = 0x8000
        with self.assertRaisesRegex(RuntimeError, "already held"):
            pointer.__enter__()
        pointer.send.assert_not_called()

    def test_foreign_hit_or_foreground_rejects_before_press(self) -> None:
        """No actual input may be sent to another application's pixels."""
        for ancestors in ([2], [1, 2]):
            with self.subTest(ancestors=ancestors):
                pointer = self.pointer()
                pointer.native.user.GetAncestor.side_effect = ancestors
                with self.assertRaises(RuntimeError):
                    pointer.press()
                pointer.send.assert_not_called()

    def test_cleanup_releases_injected_button_and_restores_untouched_cursor(self) -> None:
        """Normal or failed gestures leave no held injected button behind."""
        pointer = self.pointer()
        pointer.pressed = True
        self.assertFalse(pointer.__exit__(RuntimeError, RuntimeError("original"), None))
        self.assertEqual(pointer.send.call_args_list[0].args, ((30, 40), 4))
        self.assertEqual(pointer.send.call_args_list[1].args, ((10, 20), 0))
        self.assertFalse(pointer.pressed)

    def test_human_movement_is_preserved_after_button_cleanup(self) -> None:
        """An intervening user cursor move is never overwritten by restoration."""
        pointer = self.pointer()
        pointer.pressed = True
        pointer.cursor.return_value = (100, 200)
        original = RuntimeError("lost window ownership")
        with self.assertRaises(ExceptionGroup) as caught:
            pointer.__exit__(RuntimeError, original, None)
        pointer.send.assert_called_once_with((100, 200), 4)
        self.assertIs(caught.exception.exceptions[0], original)
        self.assertIn("human position is preserved", str(caught.exception.exceptions[1]))

    def test_virtual_desktop_coordinates_and_failed_insertion(self) -> None:
        """Signed monitors map to the full virtual desktop and failed SendInput rejects."""
        pointer = self.pointer()
        pointer.send = NativePointer.send.__get__(pointer)
        pointer.native.user.GetSystemMetrics.side_effect = lambda index: {76: -1920, 77: 0, 78: 3840, 79: 1080}[index]
        events = []

        def insert(count, event, size):
            import ctypes
            packet = ctypes.cast(event, ctypes.POINTER(MouseEvent)).contents
            events.append((packet.payload.mouse.x, packet.payload.mouse.y, packet.payload.mouse.flags, size))
            return 1

        pointer.native.user.SendInput.side_effect = insert
        pointer.send((-1920, 0), 2)
        self.assertEqual(events[0][:3], (0, 0, 0xE003))
        with self.assertRaisesRegex(RuntimeError, "outside"):
            pointer.send((-1921, 0), 0)
        pointer.native.user.SendInput.side_effect = None
        pointer.native.user.SendInput.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "did not insert"):
            pointer.send((0, 0), 2)


class CompositorObservationTests(unittest.TestCase):
    """Keep paint recognition real while faking external frames, process ownership and time."""

    def setUp(self) -> None:
        """Provide distinct empty and saturated frames from an owned compositor."""
        self.blank = Image.new("RGB", (558, 558))
        self.painted = self.blank.copy()
        for offset in range(441):
            self.painted.putpixel((100 + offset % 21, 30 + offset // 21),
                                  (200 + offset % 32, 10 + offset % 47, 20 + offset % 64))
        self.native = Mock()
        self.client = Mock(width=558, height=558, scale=1, dpi=96)
        self.native.client.return_value = self.client
        self.native.occluded.return_value = False
        self.native.user.GetAncestor.return_value = 1
        self.native.user.GetForegroundWindow.return_value = 1
        self.native.exposure_state.return_value = {"foreground_owned": False, "occluding_windows": [{"hwnd": 2, "pid": 99}]}
        self.process = Mock(pid=42)
        self.process.poll.return_value = None

    def test_blank_then_painted_compositor_qualifies_at_second_observation(self) -> None:
        """An asynchronous show/fade frame must be followed by real compositor paint."""
        self.native.capture.side_effect = [self.blank, self.painted]
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, .01, .02, .1, .2, .3]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            image, observed, exposed = observe_compositor(self.native, 1, self.process, 1)
        self.assertIs(image, self.painted)
        self.assertEqual(observed, .3)
        self.assertEqual(self.native.capture.call_count, 2)
        self.assertEqual(exposed, .01)
        self.assertEqual(self.native.client.call_count, 5)
        for call in self.native.capture.call_args_list:
            self.assertEqual(call.kwargs, {"compositor": True})

    def test_never_painted_compositor_rejects_at_deadline(self) -> None:
        """A valid PrintWindow frame cannot replace missing desktop compositor paint."""
        self.native.capture.return_value = self.blank
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, .01, .02, .1, 2]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "within the deadline"):
                observe_compositor(self.native, 1, self.process, 1)
        self.assertEqual(self.native.capture.call_count, 1)

    def test_paint_returned_after_deadline_cannot_qualify(self) -> None:
        """A late external capture remains a failed startup observation."""
        self.native.capture.return_value = self.painted
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, .01, .02, 2, 3]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "within the deadline"):
                observe_compositor(self.native, 1, self.process, 1)

    def test_occlusion_and_lost_ownership_are_terminal(self) -> None:
        """Never retry away a foreign window or an obstructed compositor capture."""
        self.native.capture.return_value = self.painted
        self.native.occluded.side_effect = [False, True]
        with self.assertRaisesRegex(RuntimeError, "occluded"):
            observe_compositor(self.native, 1, self.process, float("inf"))
        self.native.occluded.side_effect = None
        self.native.occluded.return_value = False
        self.native.client.side_effect = [self.client, self.client, RuntimeError("Window ownership changed")]
        with self.assertRaisesRegex(RuntimeError, "ownership changed"):
            observe_compositor(self.native, 1, self.process, float("inf"))

    def test_missing_or_incomplete_capture_is_terminal(self) -> None:
        """No missing frame may be mistaken for a temporary blank desktop frame."""
        for frame in (None, Image.new("RGB", (1, 1))):
            with self.subTest(frame=frame):
                self.native.capture.return_value = frame
                with self.assertRaisesRegex(RuntimeError, "complete owned client frame"):
                    observe_compositor(self.native, 1, self.process, float("inf"))

    def test_exited_process_cannot_qualify(self) -> None:
        """Do not capture a window after the owned fixture exits."""
        self.process.poll.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "within the deadline"):
            observe_compositor(self.native, 1, self.process, float("inf"))
        self.native.capture.assert_not_called()

    def test_compositor_failure_preserves_specific_and_outer_receipts(self) -> None:
        """An observation failure leaves reviewable cause receipts without a pass marker."""
        self.native.capture.return_value = self.painted
        self.native.find_window.return_value = 1
        self.native.attributes.return_value = {}
        self.native.placement.return_value = {}
        self.process.communicate.return_value = (b"", b"")
        with tempfile.TemporaryDirectory(prefix="compositor-failure-") as directory:
            output = Path(directory)
            arguments = argparse.Namespace(output=output, executable=output / "fixture.exe",
                                           duration_ms=5000, locale="en", light=True,
                                           reduced_motion=True, compositor=True)
            with patch("tools.splash_parity.native_windows.NativeWindows", return_value=self.native), \
                    patch("tools.splash_parity.native_windows.subprocess.Popen", return_value=self.process), \
                    patch("tools.splash_parity.native_windows.observe_compositor",
                          side_effect=RuntimeError("Compositor deadline fixture")):
                with self.assertRaisesRegex(RuntimeError, "deadline fixture"):
                    qualify(arguments)
            receipt = json.loads((output / "compositor.json").read_text(encoding="utf-8"))
            failure = json.loads((output / "failure.json").read_text(encoding="utf-8"))
            self.assertFalse(receipt["captured"])
            self.assertEqual(receipt["reason"], failure["error"])
            self.assertEqual(receipt["error_type"], "RuntimeError")
            self.assertFalse((output / "native-compositor.png").exists())

    def test_initial_pending_exposure_then_owned_foreground_is_accepted(self) -> None:
        """Visible-style window discovery may precede actual asynchronous foreground exposure."""
        self.native.user.GetAncestor.side_effect = [2, 1]
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, .1, .2]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            self.assertEqual(observe_exposure(self.native, 1, self.process, 1), .2)
        self.native.capture.assert_not_called()

    def test_never_exposed_and_foreign_ownership_reject_before_capture(self) -> None:
        """Waiting for initial show neither accepts an occluded window nor suppresses identity loss."""
        self.native.occluded.return_value = True
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, 2]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            with self.assertRaisesRegex(ExposureFailure, "foreground and unoccluded") as failed:
                observe_exposure(self.native, 1, self.process, 1)
        self.assertEqual(failed.exception.state["occluding_windows"], [{"hwnd": 2, "pid": 99}])
        self.native.client.side_effect = RuntimeError("Foreign window ownership")
        with self.assertRaisesRegex(RuntimeError, "Foreign window ownership"):
            observe_exposure(self.native, 1, self.process, float("inf"))
        self.native.capture.assert_not_called()


class CaptionObservationTests(unittest.TestCase):
    """Fake only external screenshots and clock, preserving the actual observation decisions."""

    def image(self, color: tuple[int, int, int]) -> Image.Image:
        """Produce a native RGB close-caption observation."""
        return Image.new("RGB", (46, 32), color)

    def test_hover_is_not_pressed_feedback(self) -> None:
        """The queued hover frame must not satisfy a press acceptance check."""
        native = Mock()
        native.capture.side_effect = [self.image((232, 17, 35)), self.image((241, 112, 122))]
        process = Mock(pid=42)
        process.poll.return_value = None
        with patch("tools.splash_parity.native_windows.time.sleep"):
            observe_caption(native, 1, process, self.image((0, 0, 0)).tobytes(), (0, 0, 46, 32), restored=False)
        self.assertEqual(native.capture.call_count, 2)

    def test_hover_only_fails_at_observation_deadline(self) -> None:
        """A stuck hover state cannot qualify close gesture handling."""
        native = Mock()
        native.capture.return_value = self.image((232, 17, 35))
        process = Mock(pid=42)
        process.poll.return_value = None
        with patch("tools.splash_parity.native_windows.time.perf_counter", side_effect=[0, 0, 2]), \
                patch("tools.splash_parity.native_windows.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "pressed"):
                observe_caption(native, 1, process, self.image((0, 0, 0)).tobytes(), (0, 0, 46, 32), restored=False)

    def test_neutral_restoration_requires_caption_equality(self) -> None:
        """Restoration remains independent of progress, pose and reduced motion."""
        neutral = self.image((0, 0, 0))
        native = Mock()
        native.capture.side_effect = [self.image((241, 112, 122)), neutral]
        process = Mock(pid=42)
        process.poll.return_value = None
        with patch("tools.splash_parity.native_windows.time.sleep"):
            observe_caption(native, 1, process, neutral.tobytes(), (0, 0, 46, 32), restored=True)
        self.assertEqual(native.capture.call_count, 2)


if __name__ == "__main__":
    unittest.main()
