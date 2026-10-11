# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Exercise real ABI/lifetime/guards while replacing only the native DLL boundary."""
import ctypes as c
import hashlib
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch
from unittest.mock import Mock

from tools.splash_parity.dxgi_abi import CaptureHelper, enroll_helper, DxgiLibrary, Request, Error, Info, Sample, validate_layout
from tools.splash_parity.dxgi_storage import DxgiCopyStorage, DxgiError
from tools.splash_parity.region_capture import RegionCapture
from tools.splash_parity.region_records import Region, RegionSample, RegionPending, validate_record
from tools.splash_parity.test_region_capture import Boundary


def wide(target, text):
    for index, character in enumerate(text):
        target[index] = ord(character)


class Library:
    def __init__(self, boundary):
        self.boundary, self.calls = boundary, []
        self.info_mutation = self.sample_mutation = self.after_attempt = None
        self.status = 0
        self.pending = 0
        self.sequence = 0
        self.close_outcomes = [0]
        self.close_errors = None
        self.error_mutation = None
        self.current = 0

    def create(self, request, context, info, error):
        self.calls.append(("create", request.acquire_timeout_ms, request.fence_timeout_ms))
        self.request = request
        context.value = 17
        info.ui_thread_id, info.qpc_frequency, info.created_qpc = 7, 1000000, self.boundary.clock
        info.monitor, info.adapter_luid_low = 21, 4
        info.output_rect.left, info.output_rect.top, info.output_rect.right, info.output_rect.bottom = -1920, 0, 0, 1080
        info.roi.left, info.roi.top = request.x, request.y
        info.roi.right, info.roi.bottom = request.x + request.width, request.y + request.height
        info.rotation, info.color_space, info.bits_per_color, info.texture_format = 1, 0, 8, 87
        info.desktop_width, info.desktop_height = 1920, 1080
        info.staging_width, info.staging_height = request.width, request.height
        wide(info.output_name, "DISPLAY1")
        wide(info.adapter_description, "Fixture adapter")
        if self.info_mutation:
            self.info_mutation(info)
        return 0

    def attempt(self, context, minimum, deadline, sample, error):
        self.calls.append(("attempt", minimum, deadline))
        sample.call_begin_qpc = self.boundary.clock + 1
        for index, name in enumerate(("acquire", "copy_submit", "fence_wait", "map_read", "release_frame")):
            getattr(sample, name).begin = sample.call_begin_qpc + index * 2
            getattr(sample, name).end = sample.call_begin_qpc + index * 2 + 1
        sample.call_end_qpc = sample.call_begin_qpc + 11
        sample.last_present_qpc = sample.acquire.begin
        sample.accumulated_frames = 1
        self.sequence += self.pending == 0
        sample.sequence = self.sequence
        sample.pending_reason = self.pending
        error.frame_owned = int(self.pending in (2, 3))
        sample.byte_count = 0 if self.pending else self.request.width * self.request.height * 4
        for index in range(sample.byte_count):
            sample.bgra[index] = index % 256
        if self.pending == 4:
            sample.last_present_qpc = 0
        elif self.pending == 5:
            sample.last_present_qpc = minimum - 1
        if self.pending in (2, 3):
            sample.release_frame.begin = sample.release_frame.end = 0
            if self.pending == 2:
                sample.map_read.begin = sample.map_read.end = 0
        if self.sample_mutation:
            self.sample_mutation(sample)
        self.boundary.clock = sample.call_end_qpc
        if self.after_attempt:
            self.after_attempt()
        if self.error_mutation:
            self.error_mutation(error)
        return 1 if self.pending else self.status

    def close(self, context, error):
        self.calls.append(("close",))
        status = self.close_outcomes.pop(0) if len(self.close_outcomes) > 1 else self.close_outcomes[0]
        self.boundary.clock += 10000
        if status == 0:
            context.value = None
        if self.close_errors:
            self.close_errors(error)
        return status


class DxgiTests(unittest.TestCase):
    def fixture(self, region=None):
        boundary = Boundary()
        library = Library(boundary)
        storage = DxgiCopyStorage(library)
        capture = RegionCapture(1, 42, region or Region(10, 12, 2, 2), creation_ticks=123,
                                boundary=boundary, storage=storage)
        return boundary, library, storage, capture

    def test_exact_header_sizes_packing_offset_and_initialized_outputs(self):
        validate_layout()
        self.assertEqual([(c.sizeof(kind), c.alignment(kind)) for kind in (Request, Error, Info, Sample)],
                         [(32, 4), (40, 4), (440, 8), (1208, 8)])
        self.assertEqual(Sample.bgra.offset, 184)
        boundary, library, storage, capture = self.fixture()
        with capture:
            self.assertEqual(library.calls[0], ("create", 5, 5))
            sample = capture.sample(deadline_ticks=1000000, minimum_present_ticks=1000)
            self.assertEqual(sample.schema_version, 2)
            self.assertEqual(sample.backend, "dxgi_v1")
            self.assertEqual(sample.bgra, bytes(range(16)))
            self.assertEqual(sample.metadata.output.roi, (-590, 212, -588, 214))
        self.assertFalse(storage.active)
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_helper_identity_strict_pair_and_prelaunch_hash(self):
        for path, digest in ((None, "0" * 64), ("fixture.dll", None)):
            self.assertRaises(ValueError, enroll_helper, path, digest)
        self.assertIsNone(enroll_helper(None, None))
        self.assertRaises(ValueError, CaptureHelper.enroll, "relative.dll", "0" * 64)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "helper.dll"
            path.write_bytes(b"exact enrolled fixture")
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            helper = enroll_helper(path, digest.upper())
            self.assertEqual(helper.sha256, digest)
            path.write_bytes(b"replacement")
            self.assertRaises(ValueError, helper.verify)
            for value in (None, "x" * 64, "0" * 63, True):
                self.assertRaises(ValueError, CaptureHelper.enroll, path, value)

    def test_verified_cdecl_restricted_loader_performs_no_native_device_calls(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "helper.dll"
            path.write_bytes(b"bounded DLL fixture")
            helper = CaptureHelper.enroll(path, hashlib.sha256(path.read_bytes()).hexdigest())
            loaded = Mock()
            with patch("tools.splash_parity.dxgi_abi.os.name", "nt"), patch("tools.splash_parity.dxgi_abi.c.CDLL", return_value=loaded) as loader:
                actual = DxgiLibrary(helper)
            loader.assert_called_once_with(str(path.resolve()), use_last_error=True, winmode=0x100 | 0x800)
            self.assertIs(actual.dll, loaded)
            self.assertEqual(loaded.ss_dxgi_v1_try_sample.restype, c.c_int32)
            for name in ("ss_dxgi_v1_create", "ss_dxgi_v1_try_sample", "ss_dxgi_v1_close"):
                getattr(loaded, name).assert_not_called()

    def test_pending_checks_all_pixel_witnesses_both_sides_and_cannot_supply_bytes(self):
        boundary, library, storage, capture = self.fixture(Region(0, 0, 16, 16))
        library.pending = 2
        with capture:
            pending = capture.sample(deadline_ticks=1000000)
            self.assertIs(type(pending), RegionPending)
            self.assertFalse(hasattr(pending, "bgra"))
            self.assertEqual(len([name for name, _ in boundary.calls if name == "hit"]), 3 * 256)
            library.sample_mutation = lambda sample: setattr(sample, "byte_count", 16)
            self.assertRaisesRegex(RuntimeError, "semantic pixel", capture.sample, deadline_ticks=1000000)

    def test_all_ready_and_pending_shared_guards_reject_before_native_attempt(self):
        for pending in (0, 2):
            for change in (lambda b: setattr(b, "alive", False), lambda b: setattr(b, "creation_value", 124),
                           lambda b: setattr(b, "foreground_value", 9), lambda b: b.pids.update({1: 99}),
                           lambda b: b.visible_values.update({1: False}), lambda b: b.minimized_values.update({1: True}),
                           lambda b: b.hit_values.update({(-590, 212): 9}),
                           lambda b: setattr(b, "geometry_value", replace(b.geometry_value, dpi=120)),
                           lambda b: setattr(b, "dpi_context_value", -2)):
                boundary, library, storage, capture = self.fixture()
                library.pending = pending
                with capture:
                    change(boundary)
                    self.assertRaises(RuntimeError, capture.sample, deadline_ticks=1000000)
                    self.assertFalse(any(item[0] == "attempt" for item in library.calls))

    def test_loss_during_ready_pending_or_native_failure_rejects_final_guard(self):
        for pending in (0, 2):
            boundary, library, storage, capture = self.fixture()
            library.pending = pending
            library.after_attempt = lambda: setattr(boundary, "foreground_value", 9)
            with capture:
                self.assertRaisesRegex(RuntimeError, "foreground", capture.sample, deadline_ticks=1000000)
        boundary, library, storage, capture = self.fixture()
        library.status = -2147467259
        library.after_attempt = lambda: setattr(boundary, "alive", False)
        with capture:
            with self.assertRaises(BaseExceptionGroup) as caught:
                capture.sample(deadline_ticks=1000000)
            self.assertIsInstance(caught.exception.exceptions[0], DxgiError)
            self.assertIn("Retained process", str(caught.exception.exceptions[1]))

    def test_creation_output_contract_unknown_or_moved_identities_reject_and_cleanup(self):
        for field, value in (("version", 2), ("size", 1), ("ui_thread_id", 8), ("qpc_frequency", 2),
                             ("monitor", 0), ("adapter_luid_low", 0), ("rotation", 2), ("color_space", 12),
                             ("advanced_color_flags", 2), ("bits_per_color", 10), ("texture_format", 28),
                             ("desktop_width", 4), ("staging_width", 3), ("created_qpc", 9000000)):
            boundary, library, storage, capture = self.fixture()
            library.info_mutation = lambda info: setattr(info, field, value)
            self.assertRaises(RuntimeError, capture.__enter__)
            self.assertFalse(storage.active)
            self.assertEqual(boundary.dpi_context_value, -1)

    def test_only_coordinated_hdr10_original_bgra_candidate_metadata_is_admitted(self):
        for flags in range(16):
            boundary, library, storage, capture = self.fixture()
            def hdr(info):
                info.color_space, info.bits_per_color, info.advanced_color_flags = 12, 10, flags
            library.info_mutation = hdr
            if flags in (2, 3):
                with capture:
                    sample = capture.sample(deadline_ticks=1000000)
                    self.assertIn("HDR fidelity unqualified", sample.metadata.output.color_contract)
                    self.assertEqual(sample.metadata.output.advanced_color_flags, flags)
            else:
                self.assertRaises(RuntimeError, capture.__enter__)
        boundary, library, storage, capture = self.fixture()
        def fp16(info):
            info.color_space, info.bits_per_color, info.advanced_color_flags, info.texture_format = 12, 10, 2, 10
        library.info_mutation = fp16
        self.assertRaises(RuntimeError, capture.__enter__)

    def test_ready_native_metadata_freshness_stages_schema_bytes_and_pointer_reject(self):
        mutations = [lambda s: setattr(s, field, value) for field, value in (
            ("version", 2), ("size", 1), ("pending_reason", 99), ("byte_count", 15),
            ("last_present_qpc", 0), ("last_present_qpc", 9999999), ("last_present_qpc", 10),
            ("sequence", 5), ("protected_content_masked", 1), ("pointer_visible", 1),
            ("rects_coalesced", 2), ("last_mouse_qpc", -1), ("accumulated_frames", 0))]
        mutations.extend((lambda s: setattr(s.acquire, "end", s.call_end_qpc + 1),
                          lambda s: setattr(s.map_read, "begin", s.copy_submit.begin),
                          lambda s: setattr(s.fence_wait, "end", 0)))
        for mutation in mutations:
            boundary, library, storage, capture = self.fixture()
            library.sample_mutation = mutation
            with capture:
                self.assertRaises((RuntimeError, ValueError), capture.sample,
                                  deadline_ticks=1000000, minimum_present_ticks=1000)

    def test_future_clock_and_original_deadline_never_accept_even_matching_pixels(self):
        boundary, library, storage, capture = self.fixture()
        with capture:
            self.assertRaises(ValueError, capture.sample)
            self.assertRaises(ValueError, capture.sample, deadline_ticks=True)
            self.assertRaisesRegex(RuntimeError, "absolute deadline", capture.sample, deadline_ticks=1020)
        boundary, library, storage, capture = self.fixture()
        with capture:
            original = library.after_attempt
            library.after_attempt = lambda: setattr(boundary, "clock", 1000001)
            self.assertRaisesRegex(RuntimeError, "deadline", capture.sample, deadline_ticks=1000000)

    def test_native_status_bool_float_unknown_and_original_cleanup_cause_are_not_accepted(self):
        for status in (True, 0.0, 2, None):
            boundary, library, storage, capture = self.fixture()
            library.status = status
            with capture:
                self.assertRaises(RuntimeError, capture.sample, deadline_ticks=1000000)
        boundary, library, storage, capture = self.fixture()
        capture.__enter__()
        library.close_errors = lambda error: setattr(error, "hresult", -2147467259)
        with self.assertRaises(ExceptionGroup) as caught:
            capture.close()
        self.assertIsInstance(caught.exception.exceptions[0], DxgiError)
        self.assertFalse(storage.active)
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_pending_cleanup_lifetime_retains_dpi_until_actual_consumption(self):
        boundary, library, storage, capture = self.fixture()
        capture.__enter__()
        library.close_outcomes = [1]
        with patch("tools.splash_parity.dxgi_storage.time.sleep"):
            self.assertRaises(ExceptionGroup, capture.close)
        self.assertTrue(storage.active)
        self.assertEqual(boundary.dpi_context_value, -4)
        self.assertIsNotNone(storage.library)
        library.close_outcomes = [1, 0]
        with patch("tools.splash_parity.dxgi_storage.time.sleep"):
            capture.close()
        self.assertFalse(storage.active)
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_current_explicit_schema_does_not_accept_historical_or_unknown_records(self):
        boundary, library, storage, capture = self.fixture()
        with capture:
            sample = capture.sample(deadline_ticks=1000000)
        for bad in (dict(bgra=sample.bgra), replace(sample, schema_version=3), replace(sample, backend="unknown")):
            self.assertRaises(RuntimeError, validate_record, bad, frequency=1000000, deadline=1000000)

    def test_failed_create_nonnull_context_has_retained_recovery_owner_and_retry(self):
        from tools.splash_parity.region_capture import _CAPTURE_OWNERS
        boundary, library, storage, capture = self.fixture()
        original = library.create
        def failed_create(*args):
            original(*args)
            args[-1].hresult = -2147467259
            return -2147467259
        library.create = failed_create
        library.close_outcomes = [1]
        with patch("tools.splash_parity.dxgi_storage.time.sleep"):
            self.assertRaises(BaseExceptionGroup, capture.__enter__)
        self.assertIs(_CAPTURE_OWNERS[id(capture)], capture)
        self.assertTrue(storage.active)
        self.assertEqual(boundary.dpi_context_value, -4)
        library.close_outcomes = [0]
        capture.close()
        self.assertNotIn(id(capture), _CAPTURE_OWNERS)
        self.assertEqual(boundary.dpi_context_value, -1)

    def test_held_resume_keeps_source_and_earlier_acquire_stage_not_current_call(self):
        boundary, library, storage, capture = self.fixture()
        library.pending = 2
        with capture:
            initial = capture.sample(deadline_ticks=1000000)
            old = initial.metadata
            library.pending = 0
            def resume(sample):
                sample.acquire.begin, sample.acquire.end = old.acquire.begin, old.acquire.end
                sample.copy_submit.begin, sample.copy_submit.end = old.copy_submit.begin, old.copy_submit.end
                sample.last_present_qpc = old.last_present_qpc
            library.sample_mutation = resume
            current = capture.sample(deadline_ticks=1000000)
            self.assertLess(current.metadata.acquire.begin, current.metadata.call.begin)
            self.assertEqual(current.metadata.last_present_qpc, initial.metadata.last_present_qpc)

    def test_resumed_frame_cannot_change_source_or_cross_new_input_floor(self):
        for mode in ("changed", "raised_floor"):
            boundary, library, storage, capture = self.fixture()
            library.pending = 2
            with capture:
                initial = capture.sample(deadline_ticks=1000000)
                old = initial.metadata
                library.pending = 0
                def resumed(sample):
                    sample.acquire.begin, sample.acquire.end = old.acquire.begin, old.acquire.end
                    sample.copy_submit.begin, sample.copy_submit.end = old.copy_submit.begin, old.copy_submit.end
                    sample.last_present_qpc = old.last_present_qpc + int(mode == "changed")
                library.sample_mutation = resumed
                self.assertRaises(RuntimeError, capture.sample, deadline_ticks=1000000,
                                  minimum_present_ticks=old.last_present_qpc + int(mode == "raised_floor"))

    def test_nonzero_desktop_source_may_predate_context_but_native_stages_may_not(self):
        boundary, library, storage, capture = self.fixture()
        library.sample_mutation = lambda sample: setattr(sample, "last_present_qpc", 1)
        with capture:
            sample = capture.sample(deadline_ticks=1000000)
            self.assertLess(sample.metadata.last_present_qpc, sample.metadata.output.created_qpc)
        boundary, library, storage, capture = self.fixture()
        library.sample_mutation = lambda sample: setattr(sample.acquire, "begin", 1)
        with capture:
            self.assertRaises(RuntimeError, capture.sample, deadline_ticks=1000000)

    def test_invalid_error_envelope_keeps_this_calls_raw_native_effects_and_consumed_cleanup(self):
        from tools.splash_parity.region_capture import _CAPTURE_OWNERS
        boundary, library, storage, capture = self.fixture()
        with capture:
            capture.sample(deadline_ticks=1000000)
            library.pending = 2
            library.error_mutation = lambda error: setattr(error, "version", 99)
            self.assertRaises(RuntimeError, capture.sample, deadline_ticks=1000000)
            evidence = capture.last_attempt["backend_evidence"]
            self.assertEqual(evidence["native_error"]["version"], 99)
            self.assertEqual(evidence["native_error"]["frame_owned"], 1)
            self.assertEqual(evidence["native_sample"]["pending_reason"], 2)
            self.assertEqual(evidence["native_sample"]["byte_count"], 0)
        boundary, library, storage, capture = self.fixture()
        capture.__enter__()
        library.close_errors = lambda error: setattr(error, "version", 99)
        self.assertRaises(ExceptionGroup, capture.close)
        self.assertFalse(storage.last_cleanup["context_retained"])
        self.assertEqual(storage.last_cleanup["native_error"]["version"], 99)
        self.assertNotIn(id(capture), _CAPTURE_OWNERS)

    def test_create_success_before_failed_clock_and_cleanup_retains_raw_info_and_complete_owner(self):
        from tools.splash_parity.region_capture import _CAPTURE_OWNERS
        boundary, library, storage, capture = self.fixture()
        original = boundary.ticks
        calls = [0]
        def ticks():
            calls[0] += 1
            if calls[0] == 1:
                raise OSError("post-create QPC failed")
            return original()
        boundary.ticks = ticks
        library.close_outcomes = [-2147467259]
        with self.assertRaises(BaseExceptionGroup):
            capture.__enter__()
        raw = capture.last_attempt["backend_evidence"]
        self.assertTrue(raw["native_call_returned"])
        self.assertTrue(raw["context_retained"])
        self.assertEqual(raw["status"], 0)
        self.assertEqual(raw["native_info"]["monitor"], 21)
        self.assertIn(id(capture), _CAPTURE_OWNERS)
        library.close_outcomes = [0]
        capture.close()
        self.assertNotIn(id(capture), _CAPTURE_OWNERS)


if __name__ == "__main__":
    unittest.main()
