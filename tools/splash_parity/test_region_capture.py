# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Keep capture/lifecycle decisions real while replacing only native boundaries."""

import unittest
from collections import Counter

from tools.splash_parity.region_capture import Geometry, Region, RegionCapture, _BitmapHeader, _BitmapInfo, _Win32Boundary


class Boundary:
    """Provide deterministic native metadata, clock, pixels and individual call failures."""

    def __init__(self) -> None:
        self.calls = []
        self.failures = {}
        self.clock = 1000
        self.thread = 7
        self.dpi_context_value = -1
        self.dpi_change_applied = True
        self.dpi_equal_override = None
        self.frequency_value = 1_000_000
        self.creation_value = 123
        self.alive = True
        self.geometry_value = Geometry(558, 558, 96, -600, 200)
        self.visible_values = {1: True}
        self.minimized_values = {}
        self.pids = {1: 42, 2: 42, 9: 99}
        self.roots = {1: 1, 2: 1, 9: 9}
        self.foreground_value = 1
        self.hit_values = {}
        self.above_values = []
        self.bound_values = {}
        self.after_copy = None
        self.after_read = None
        self.read_override = None
        self.tick_override = None

    def call(self, name, *arguments):
        self.calls.append((name, arguments))
        self.clock += 1
        error = self.failures.get(name)
        if error is not None:
            raise error

    def thread_id(self):
        self.call("thread_id")
        return self.thread

    def set_dpi_context(self, context):
        self.call("dpi_enter" if context == -4 else "dpi_restore", context)
        previous = self.dpi_context_value
        if self.dpi_change_applied:
            self.dpi_context_value = context
        return previous

    def dpi_context(self):
        self.call("dpi_context")
        return self.dpi_context_value

    def dpi_equal(self, left, right):
        self.call("dpi_equal", left, right)
        return self.dpi_equal_override(left, right) if self.dpi_equal_override else left == right

    def frequency(self):
        self.call("frequency")
        return self.frequency_value

    def ticks(self):
        self.call("ticks")
        return self.tick_override() if self.tick_override else self.clock

    def open_process(self, pid):
        self.call("open_process", pid)
        return 10

    def process_alive(self, handle):
        self.call("process_alive", handle)
        return self.alive

    def creation(self, handle):
        self.call("creation", handle)
        return self.creation_value

    def window_pid(self, hwnd):
        self.call("window_pid", hwnd)
        return self.pids[hwnd]

    def root(self, hwnd):
        self.call("root", hwnd)
        return self.roots[hwnd]

    def foreground(self):
        self.call("foreground")
        return self.foreground_value

    def visible(self, hwnd):
        self.call("visible", hwnd)
        return self.visible_values.get(hwnd, True)

    def minimized(self, hwnd):
        self.call("minimized", hwnd)
        return self.minimized_values.get(hwnd, False)

    def geometry(self, hwnd):
        self.call("geometry", hwnd)
        return self.geometry_value

    def above(self, hwnd):
        self.call("above", hwnd)
        return self.above_values

    def bounds(self, hwnd):
        self.call("bounds", hwnd)
        return self.bound_values[hwnd]

    def hit(self, x, y):
        self.call("hit", x, y)
        return self.hit_values.get((x, y), 1)

    def screen_dc(self):
        self.call("screen_dc")
        return 20

    def memory_dc(self, screen):
        self.call("memory_dc", screen)
        return 30

    def dib(self, screen, width, height):
        self.call("dib", screen, width, height)
        return 40, 50

    def select(self, dc, bitmap):
        self.call("select" if bitmap == 40 else "restore", dc, bitmap)
        return 60

    def copy(self, *arguments):
        self.call("copy", *arguments)
        if self.after_copy:
            self.after_copy()

    def gdi_flush(self):
        self.call("gdi_flush")

    def dwm_flush(self):
        self.call("dwm_flush")

    def read(self, pointer, size):
        self.call("read", pointer, size)
        if self.after_read:
            self.after_read()
        return self.read_override if self.read_override is not None else bytes(range(16)) * (size // 16)

    def delete_bitmap(self, bitmap):
        self.call("delete_bitmap", bitmap)

    def delete_dc(self, dc):
        self.call("delete_dc", dc)

    def release_screen(self, dc):
        self.call("release_screen", dc)

    def close_process(self, handle):
        self.call("close_process", handle)


class RegionCaptureTests(unittest.TestCase):
    """Reject uncertain source pixels/timings and prove bounded reusable ownership."""

    def capture(self, boundary=None, region=None, **options):
        return RegionCapture(1, 42, region or Region(10, 12, 2, 2), creation_ticks=123,
                             boundary=boundary or Boundary(), **options)

    def test_region_and_supplied_identity_reject_invalid_bounds_without_native_calls(self):
        for values in [(0, 0, 1, 2), (0, 0, 2, 17), (-1, 0, 2, 2), (True, 0, 2, 2), (0, 0, 2.0, 2)]:
            with self.subTest(values=values), self.assertRaises(ValueError):
                Region(*values)
        for options in [{"hwnd": 0}, {"pid": 0x100000000}, {"creation_ticks": 0}, {"flush": 1}]:
            boundary = Boundary()
            values = dict(hwnd=1, pid=42, region=Region(0, 0, 2, 2), creation_ticks=123, boundary=boundary)
            values.update(options)
            with self.subTest(options=options), self.assertRaises(ValueError):
                RegionCapture(**values)
            self.assertEqual(boundary.calls, [])

    def test_reuses_resources_and_returns_immutable_bgra_with_all_observation_brackets(self):
        boundary = Boundary()
        with self.capture(boundary) as capture:
            first, second = capture.sample(), capture.sample()
            self.assertEqual(first.bgra, bytes(range(16)))
            self.assertEqual(first.geometry, Geometry(558, 558, 96, -600, 200))
            self.assertEqual((first.hwnd, first.pid, first.creation_ticks, first.qpc_frequency), (1, 42, 123, 1_000_000))
            self.assertIsNone(first.flush_begin_ticks)
            self.assertIsNone(first.flush_end_ticks)
            order = [first.begin_ticks, first.ownership_before_end_ticks, first.copy_begin_ticks,
                     first.copy_end_ticks, first.read_end_ticks, first.end_ticks, second.begin_ticks]
            self.assertEqual(order, sorted(order))
            self.assertGreater(first.end_ticks - first.read_end_ticks, first.copy_end_ticks - first.copy_begin_ticks)
        counts = Counter(name for name, _ in boundary.calls)
        for operation in ("open_process", "screen_dc", "memory_dc", "dib", "restore", "delete_bitmap", "delete_dc", "release_screen", "close_process"):
            self.assertEqual(counts[operation], 1, operation)
        self.assertEqual(counts["copy"], 2)
        self.assertEqual(counts["gdi_flush"], 2)
        self.assertEqual(counts["hit"], 20)
        self.assertEqual(counts["dwm_flush"], 0)
        self.assertEqual(counts["dpi_enter"], 1)
        self.assertEqual(counts["dpi_restore"], 1)
        self.assertEqual(boundary.dpi_context_value, -1)
        self.assertEqual([args for name, args in boundary.calls if name == "copy"], [(30, 20, -590, 212, 2, 2)] * 2)
        capture.close()
        with self.assertRaises(RuntimeError):
            capture.sample()
        with self.assertRaises(RuntimeError):
            capture.__enter__()

    def test_optional_flush_keeps_its_actual_cost_bracket(self):
        with self.capture(flush=True) as capture:
            sample = capture.sample()
        self.assertLess(sample.ownership_before_end_ticks, sample.flush_begin_ticks)
        self.assertLess(sample.flush_begin_ticks, sample.flush_end_ticks)
        self.assertLess(sample.flush_end_ticks, sample.copy_begin_ticks)

    def test_all_region_pixels_are_witnessed_and_owned_native_child_is_allowed(self):
        boundary = Boundary()
        boundary.hit_values[(-588, 214)] = 2
        with self.capture(boundary, Region(10, 12, 4, 4)) as capture:
            self.assertEqual(len(capture.sample().bgra), 64)
            boundary.hit_values[(-588, 214)] = 9
            with self.assertRaisesRegex(RuntimeError, "pixel"):
                capture.sample()
        hits = [args for name, args in boundary.calls if name == "hit"]
        self.assertIn((-588, 214), hits)

    def test_child_with_foreign_pid_cannot_borrow_the_owned_root(self):
        boundary = Boundary()
        boundary.hit_values[(-590, 212)] = 2
        boundary.pids[2] = 99
        with self.assertRaisesRegex(RuntimeError, "pixel"):
            self.capture(boundary).__enter__()

    def test_hidden_minimized_foreground_process_and_geometry_changes_reject_before_copy(self):
        changes = [lambda b: setattr(b, "alive", False), lambda b: setattr(b, "creation_value", 124),
                   lambda b: b.pids.update({1: 99}), lambda b: b.roots.update({1: 9}),
                   lambda b: b.visible_values.update({1: False}), lambda b: b.minimized_values.update({1: True}),
                   lambda b: setattr(b, "foreground_value", 9), lambda b: setattr(b, "foreground_value", 0),
                   lambda b: setattr(b, "geometry_value", Geometry(558, 558, 96, -599, 200)),
                   lambda b: setattr(b, "geometry_value", Geometry(558, 558, 120, -600, 200)),
                   lambda b: setattr(b, "geometry_value", Geometry(557, 558, 96, -600, 200))]
        for change in changes:
            with self.subTest(change=change):
                boundary = Boundary()
                with self.capture(boundary) as capture:
                    change(boundary)
                    with self.assertRaises(RuntimeError):
                        capture.sample()
                self.assertNotIn("copy", [name for name, _ in boundary.calls])

    def test_change_during_capture_rejects_without_returning_foreign_or_moved_pixels(self):
        for change in [lambda b: setattr(b, "foreground_value", 9),
                       lambda b: setattr(b, "geometry_value", Geometry(558, 558, 96, -599, 200)),
                       lambda b: setattr(b, "alive", False)]:
            boundary = Boundary()
            with self.capture(boundary) as capture:
                boundary.after_copy = lambda: change(boundary)
                with self.assertRaises(RuntimeError):
                    capture.sample()

    def test_occluding_above_window_rejects_even_if_hit_testing_passes_through_it(self):
        boundary = Boundary()
        with self.capture(boundary) as capture:
            boundary.above_values = [9]
            boundary.bound_values[9] = (-589, 213, -588, 214)
            with self.assertRaisesRegex(RuntimeError, "occludes"):
                capture.sample()
            boundary.bound_values[9] = (0, 0, 10, 10)
            capture.sample()

    def test_region_outside_client_and_native_coordinate_overflow_reject(self):
        for region, geometry in [(Region(557, 0, 2, 2), Geometry(558, 558, 96, 0, 0)),
                                 (Region(0, 0, 2, 2), Geometry(558, 558, 96, 0x7fffffff, 0))]:
            boundary = Boundary()
            boundary.geometry_value = geometry
            with self.assertRaises(RuntimeError):
                self.capture(boundary, region).__enter__()

    def test_ffi_header_uses_fixed_windows_widths_and_top_down_description_size(self):
        import ctypes
        self.assertEqual(ctypes.sizeof(_BitmapHeader), 40)

    def test_native_dib_descriptor_is_top_down_32bit_and_pointer_read_is_exact(self):
        import ctypes
        from unittest.mock import Mock
        native = object.__new__(_Win32Boundary)
        native._gdi = Mock()
        backing = (ctypes.c_ubyte * 16)(*range(16))
        descriptors = []

        def create(screen, description, usage, pixels, section, offset):
            header = ctypes.cast(description, ctypes.POINTER(_BitmapInfo)).contents.header
            descriptors.append((header.size, header.width, header.height, header.planes, header.bits,
                                header.compression, header.image_size, usage, section, offset))
            ctypes.cast(pixels, ctypes.POINTER(ctypes.c_void_p)).contents.value = ctypes.addressof(backing)
            return 40

        native._gdi.CreateDIBSection.side_effect = create
        bitmap, pointer = native.dib(20, 2, 2)
        self.assertEqual(bitmap, 40)
        self.assertEqual(descriptors, [(40, 2, -2, 1, 32, 0, 16, 0, None, 0)])
        self.assertEqual(native.read(pointer, 16), bytes(range(16)))

    def test_largest_region_has_one_kib_snapshot_and_all_256_pixel_witnesses_per_check(self):
        boundary = Boundary()
        with self.capture(boundary, Region(0, 0, 16, 16)) as capture:
            self.assertEqual(len(capture.sample().bgra), 1024)
        self.assertEqual(Counter(name for name, _ in boundary.calls)["hit"], 256 * 3)

    def test_failed_occluder_bounds_are_never_treated_as_unoccluded(self):
        boundary = Boundary()
        with self.capture(boundary) as capture:
            boundary.above_values = [9]
            boundary.failures["bounds"] = OSError("unknown occluder bounds")
            with self.assertRaisesRegex(OSError, "unknown occluder"):
                capture.sample()

    def test_every_native_observation_failure_remains_a_failure(self):
        operations = ["thread_id", "dpi_context", "dpi_equal", "process_alive", "creation", "window_pid", "root", "visible", "minimized",
                      "foreground", "geometry", "above", "hit", "ticks", "copy", "gdi_flush", "read", "dwm_flush"]
        for operation in operations:
            boundary = Boundary()
            with self.subTest(operation=operation), self.capture(boundary, flush=True) as capture:
                failure = OSError(f"failed {operation}")
                boundary.failures[operation] = failure
                try:
                    with self.assertRaises(OSError) as caught:
                        capture.sample()
                    self.assertIs(caught.exception, failure)
                finally:
                    boundary.failures.clear()

    def test_reversed_ticks_and_wrong_byte_length_never_qualify(self):
        for override in [b"", b"too short", bytearray(16)]:
            boundary = Boundary()
            with self.capture(boundary) as capture:
                boundary.read_override = override
                with self.assertRaisesRegex(RuntimeError, "exact bounded"):
                    capture.sample()
        boundary = Boundary()
        with self.capture(boundary) as capture:
            value = iter([100, 101, 102, 103, 104, 99])
            boundary.tick_override = lambda: next(value)
            with self.assertRaisesRegex(RuntimeError, "timing"):
                capture.sample()

    def test_constructor_failure_cleans_every_resource_already_acquired(self):
        for failed, released in [("frequency", []), ("open_process", []), ("creation", ["close_process"]),
                                 ("screen_dc", ["close_process"]), ("memory_dc", ["release_screen", "close_process"]),
                                 ("dib", ["delete_dc", "release_screen", "close_process"]),
                                 ("select", ["delete_bitmap", "delete_dc", "release_screen", "close_process"])]:
            boundary = Boundary()
            boundary.failures[failed] = OSError(failed)
            with self.subTest(failed=failed), self.assertRaises(OSError):
                self.capture(boundary).__enter__()
            actual = [name for name, _ in boundary.calls if name in ("delete_bitmap", "delete_dc", "release_screen", "close_process")]
            self.assertEqual(actual, released)

    def test_constructor_primary_and_cleanup_failure_are_both_retained(self):
        boundary = Boundary()
        primary, cleanup = OSError("DIB creation"), OSError("DC teardown")
        boundary.failures.update(dib=primary, delete_dc=cleanup)
        capture = self.capture(boundary)
        with self.assertRaises(ExceptionGroup) as caught:
            capture.__enter__()
        self.assertIs(caught.exception.exceptions[0], primary)
        self.assertIs(caught.exception.exceptions[1].exceptions[0], cleanup)
        self.assertIsNotNone(capture._dc)
        boundary.failures.clear()
        capture.close()

    def test_invalid_dib_pointer_keeps_bitmap_owned_through_failed_cleanup(self):
        from unittest.mock import Mock
        boundary = Boundary()
        boundary.dib = Mock(return_value=(40, None))
        boundary.failures["delete_bitmap"] = OSError("bitmap teardown")
        capture = self.capture(boundary)
        with self.assertRaises(ExceptionGroup) as caught:
            capture.__enter__()
        self.assertIn("bitmap storage", str(caught.exception.exceptions[0]))
        self.assertEqual(capture._bitmap, 40)
        boundary.failures.clear()
        capture.close()
        self.assertIsNone(capture._bitmap)

    def test_bitmap_delete_failure_remains_owned_without_automatic_retry(self):
        boundary = Boundary()
        capture = self.capture(boundary).__enter__()
        boundary.failures["delete_bitmap"] = OSError("bitmap teardown")
        with self.assertRaises(ExceptionGroup):
            capture.close()
        self.assertEqual(Counter(name for name, _ in boundary.calls)["delete_bitmap"], 1)
        self.assertIsNotNone(capture._bitmap)
        self.assertIsNone(capture._dc)
        boundary.failures.clear()
        capture.close()
        self.assertIsNone(capture._bitmap)

    def test_cleanup_preserves_primary_and_all_cleanup_causes_and_allows_explicit_retry(self):
        boundary = Boundary()
        capture = self.capture(boundary)
        primary = RuntimeError("original observation")
        with self.assertRaises(ExceptionGroup) as caught:
            with capture:
                boundary.failures.update({name: OSError(name) for name in ("restore", "delete_dc", "release_screen", "close_process")})
                raise primary
        self.assertIs(caught.exception.exceptions[0], primary)
        cleanup = caught.exception.exceptions[1]
        self.assertEqual([str(error) for error in cleanup.exceptions], ["restore", "delete_dc", "release_screen", "close_process"])
        self.assertIsNotNone(capture._bitmap)
        boundary.failures.clear()
        capture.close()
        self.assertIsNone(capture._bitmap)
        self.assertIsNone(capture._dc)
        self.assertIsNone(capture._screen)
        self.assertIsNone(capture._process)

    def test_failed_restore_deletes_dc_before_bitmap_without_retrying_the_restore(self):
        boundary = Boundary()
        with self.assertRaises(ExceptionGroup):
            with self.capture(boundary):
                boundary.failures["restore"] = OSError("restore")
        cleanup = [name for name, _ in boundary.calls if name in ("restore", "delete_dc", "delete_bitmap", "release_screen", "close_process")]
        self.assertEqual(cleanup, ["restore", "delete_dc", "delete_bitmap", "release_screen", "close_process"])

    def test_wrong_thread_rejects_sample_and_cleanup_until_owner_performs_cleanup(self):
        boundary = Boundary()
        capture = self.capture(boundary).__enter__()
        boundary.thread = 8
        with self.assertRaisesRegex(RuntimeError, "owning native thread"):
            capture.sample()
        with self.assertRaisesRegex(RuntimeError, "owning native thread"):
            capture.close()
        self.assertEqual(boundary.dpi_context_value, -4)
        boundary.thread = 7
        capture.close()
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_thread_dpi_guard_precedes_geometry_and_restores_after_all_resources(self):
        boundary = Boundary()
        with self.capture(boundary) as capture:
            self.assertEqual(boundary.dpi_context_value, -4)
            capture.sample()
        names = [name for name, _ in boundary.calls]
        self.assertLess(names.index("dpi_enter"), names.index("geometry"))
        self.assertGreater(names.index("dpi_restore"), names.index("close_process"))
        self.assertGreater(names.index("dpi_restore"), names.index("release_screen"))
        self.assertGreater(names.index("dpi_restore"), names.index("delete_bitmap"))
        self.assertGreater(names.index("dpi_restore"), names.index("delete_dc"))

    def test_failed_dpi_set_or_verification_rejects_before_native_capture_resources(self):
        boundary = Boundary()
        boundary.failures["dpi_enter"] = OSError("DPI set failed")
        with self.assertRaisesRegex(OSError, "DPI set failed"):
            self.capture(boundary).__enter__()
        self.assertEqual(boundary.dpi_context_value, -1)
        self.assertNotIn("open_process", [name for name, _ in boundary.calls])
        boundary = Boundary()
        boundary.dpi_change_applied = False
        with self.assertRaisesRegex(RuntimeError, "Per Monitor v2"):
            self.capture(boundary).__enter__()
        self.assertEqual(boundary.dpi_context_value, -1)
        self.assertIn("dpi_restore", [name for name, _ in boundary.calls])

    def test_null_previous_dpi_context_rejects_before_geometry_or_resources(self):
        from unittest.mock import Mock
        boundary = Boundary()
        boundary.set_dpi_context = Mock(return_value=None)
        with self.assertRaisesRegex(RuntimeError, "could not be established"):
            self.capture(boundary).__enter__()
        self.assertNotIn("geometry", [name for name, _ in boundary.calls])
        self.assertNotIn("open_process", [name for name, _ in boundary.calls])

    def test_constructor_failure_restores_caller_context(self):
        boundary = Boundary()
        boundary.failures["dib"] = OSError("DIB creation")
        with self.assertRaisesRegex(OSError, "DIB creation"):
            self.capture(boundary).__enter__()
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_failed_resource_cleanup_retains_dpi_guard_until_explicit_retry(self):
        boundary = Boundary()
        capture = self.capture(boundary).__enter__()
        boundary.failures["delete_bitmap"] = OSError("bitmap teardown")
        with self.assertRaises(ExceptionGroup):
            capture.close()
        self.assertEqual(boundary.dpi_context_value, -4)
        self.assertEqual(capture._previous_dpi, -1)
        self.assertNotIn("dpi_restore", [name for name, _ in boundary.calls])
        boundary.failures.clear()
        capture.close()
        self.assertEqual(boundary.dpi_context_value, -1)
        self.assertIsNone(capture._previous_dpi)

    def test_restore_failure_retains_primary_cause_and_old_context_for_retry(self):
        boundary = Boundary()
        capture = self.capture(boundary)
        primary = RuntimeError("observation failed")
        with self.assertRaises(ExceptionGroup) as caught:
            with capture:
                boundary.failures["dpi_restore"] = OSError("DPI restore failed")
                raise primary
        self.assertIs(caught.exception.exceptions[0], primary)
        self.assertEqual(str(caught.exception.exceptions[1].exceptions[0]), "DPI restore failed")
        self.assertEqual(capture._previous_dpi, -1)
        self.assertEqual(boundary.dpi_context_value, -4)
        boundary.failures.clear()
        capture.close()
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_changed_dpi_context_during_sample_and_failed_restore_verification_reject(self):
        boundary = Boundary()
        capture = self.capture(boundary).__enter__()
        boundary.after_copy = lambda: setattr(boundary, "dpi_context_value", -2)
        with self.assertRaisesRegex(RuntimeError, "Per Monitor v2"):
            capture.sample()
        boundary.dpi_change_applied = False
        with self.assertRaises(ExceptionGroup) as caught:
            capture.close()
        self.assertIn("restoration", str(caught.exception.exceptions[0]))
        self.assertEqual(capture._previous_dpi, -1)
        boundary.dpi_change_applied = True
        capture.close()
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_native_dpi_context_comparison_uses_api_flags_semantics(self):
        from unittest.mock import Mock
        native = object.__new__(_Win32Boundary)
        native._user = Mock()
        native._user.AreDpiAwarenessContextsEqual.return_value = 1
        self.assertTrue(native.dpi_equal(0x12340004, -4))
        native._user.AreDpiAwarenessContextsEqual.assert_called_once_with(0x12340004, -4)


if __name__ == "__main__":
    unittest.main()
