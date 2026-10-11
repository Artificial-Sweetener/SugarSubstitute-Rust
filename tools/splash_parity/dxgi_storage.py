# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Own one verified DXGI context and validate its actual native copy evidence.

Window ownership rules stay exclusively in RegionCapture. Finite native waits
cannot interrupt a stalled driver call; an expired cleanup bound is a failure,
not evidence that native storage or outstanding GPU work stopped.
"""
from __future__ import annotations
import ctypes as c
import time
try:
    from .dxgi_abi import Request, Error, Info, Sample, initialized
    from .region_records import CopyResult, DxgiOutput, DxgiMetadata, Interval, integer
except ImportError:
    from dxgi_abi import Request, Error, Info, Sample, initialized
    from region_records import CopyResult, DxgiOutput, DxgiMetadata, Interval, integer

PENDING = ("ready", "acquire_timeout", "fence_pending", "map_pending", "pointer_only", "stale", "deadline")
STAGES = ("acquire", "copy_submit", "fence_wait", "map_read", "release_frame")


class DxgiError(RuntimeError):
    """Preserve original HRESULT, operation and independent native cleanup cause."""
    def __init__(self, operation, status, native):
        self.operation, self.status, self.native = operation, status, native
        super().__init__(f"DXGI {operation}: HRESULT {status:#x}; native={native}")


def envelope(value, kind):
    if value.version != 1 or value.size != c.sizeof(kind):
        raise RuntimeError(f"Unknown DXGI {kind.__name__} version or size")


def native_error(value):
    envelope(value, Error)
    data = {name: getattr(value, name) for name, _ in Error._fields_ if name not in ("version", "size")}
    if data["operation"] > 14 or data["cleanup_operation"] > 14:
        raise RuntimeError("Unknown DXGI error operation")
    for name in ("poisoned", "frame_owned", "gpu_pending", "event_registered"):
        integer(data[name], 0, 1, name)
    return data


def raw_error(value):
    """Retain fixed POD evidence before any fallible semantic validation."""
    return {name: getattr(value, name) for name, _ in Error._fields_}


def raw_sample(value):
    data = {name: getattr(value, name) for name, _ in Sample._fields_ if name not in (*STAGES, "bgra")}
    data.update({name: {"begin": getattr(value, name).begin, "end": getattr(value, name).end} for name in STAGES})
    data["bgra_prefix"] = bytes(value.bgra[:min(value.byte_count, 1024)]).hex()
    return data


def raw_info(value):
    data = {name: getattr(value, name) for name, _ in Info._fields_
            if name not in ("output_rect", "roi", "output_name", "adapter_description")}
    data.update(output_rect=rectangle(value.output_rect), roi=rectangle(value.roi),
                output_name_utf16=list(value.output_name), adapter_description_utf16=list(value.adapter_description))
    return data


def status_result(status, error, operation):
    data = native_error(error)
    if type(status) is not int or status not in (0, 1) and not -0x80000000 <= status < 0:
        raise RuntimeError("Unknown DXGI HRESULT return type or positive status")
    if status < 0 or data["hresult"] < 0 or data["cleanup_hresult"] < 0:
        raise DxgiError(operation, status, data)
    return data


def rectangle(value):
    return tuple(getattr(value, name) for name, _ in value._fields_)


def wide_string(value):
    units = list(value)
    if 0 not in units:
        raise RuntimeError("DXGI output string is not bounded and terminated")
    units = units[:units.index(0)]
    if not units:
        raise RuntimeError("DXGI output identity string is empty")
    return b"".join(unit.to_bytes(2, "little") for unit in units).decode("utf-16-le", "strict")


def output_info(info, roi, frequency, thread, after_create):
    envelope(info, Info)
    output = rectangle(info.output_rect)
    if rectangle(info.roi) != roi or not output[0] <= roi[0] < roi[2] <= output[2] or not output[1] <= roi[1] < roi[3] <= output[3]:
        raise RuntimeError("DXGI ROI does not belong wholly to its exact enrolled output")
    if info.ui_thread_id != thread or info.qpc_frequency != frequency or not 0 < info.created_qpc <= after_create:
        raise RuntimeError("DXGI creation thread or QPC identity differs")
    if not info.monitor or not (info.adapter_luid_low or info.adapter_luid_high):
        raise RuntimeError("DXGI output lacks native monitor/adapter identity")
    sdr = info.color_space == 0 and info.bits_per_color == 8 and info.advanced_color_flags in (0, 1, 8, 9)
    hdr10 = info.color_space == 12 and info.bits_per_color == 10 and info.advanced_color_flags in (2, 3)
    if info.rotation != 1 or not (sdr or hdr10) or info.texture_format != 87:
        raise RuntimeError("DXGI output has unsupported rotation, advanced color or format")
    if ((info.desktop_width, info.desktop_height) != (output[2] - output[0], output[3] - output[1])
            or (info.staging_width, info.staging_height) != (roi[2] - roi[0], roi[3] - roi[1])):
        raise RuntimeError("DXGI native output/staging dimensions differ")
    values = {name: getattr(info, name) for name, _ in Info._fields_ if name not in ("version", "size")}
    values.update(output_rect=output, roi=roi, output_name=wide_string(info.output_name),
                  adapter_description=wide_string(info.adapter_description),
                  color_contract="original DuplicateOutput, acquired BGRA8, OS desktop color processing; HDR fidelity unqualified" if hdr10
                  else "original DuplicateOutput, acquired BGRA8, SDR desktop color processing")
    return DxgiOutput(**values)


class DxgiCopyStorage:
    """Creating-thread context with one acquire/wait/map attempt per sample call."""
    backend = "dxgi_v1"

    def __init__(self, library):
        self.library, self.context = library, c.c_void_p()
        self.info = self.clock = self.frequency = self.region = None
        self.previous_call_end = 0
        self.previous_sequence = 0
        self.last_creation = self.last_attempt = self.last_cleanup = None
        self._held = None

    @property
    def active(self):
        return bool(self.context.value)

    def open(self, geometry, region, frequency, thread, clock):
        if self.active or self.info is not None:
            raise RuntimeError("A DXGI storage lifetime cannot be opened again")
        self.clock, self.frequency, self.region = clock, frequency, region
        x, y = geometry.screen_x + region.x, geometry.screen_y + region.y
        request = initialized(Request)
        request.x, request.y, request.width, request.height = x, y, region.width, region.height
        request.acquire_timeout_ms = request.fence_timeout_ms = 5
        info, error = initialized(Info), initialized(Error)
        self.last_creation = {"native_call_returned": False}
        try:
            status = self.library.create(request, self.context, info, error)
        except BaseException:
            self.last_creation.update(context_retained=self.active, native_info=raw_info(info), native_error=raw_error(error))
            raise
        self.last_creation = {"status": status, "native_call_returned": True, "context_retained": self.active,
                              "native_info": raw_info(info), "native_error": raw_error(error)}
        # Native context ownership is observed before any failable validation.
        status_result(status, error, "create")
        if status != 0 or not self.active:
            raise RuntimeError("DXGI create did not return a live owned context")
        self.info = output_info(info, (x, y, x + region.width, y + region.height), frequency, thread, clock())

    def attempt(self, minimum, deadline):
        integer(minimum, 0, 0x7fffffffffffffff, "source floor")
        integer(deadline, 1, 0x7fffffffffffffff, "absolute capture deadline")
        sample, error = initialized(Sample), initialized(Error)
        self.last_attempt = {"operation": "try_sample", "native_call_returned": False}
        status = self.library.attempt(self.context, minimum, deadline, sample, error)
        self.last_attempt = {"status": status, "native_call_returned": True, "native_sample": raw_sample(sample),
                             "native_error": raw_error(error)}
        effects = status_result(status, error, "try_sample")
        envelope(sample, Sample)
        integer(sample.pending_reason, 0, len(PENDING) - 1, "pending reason")
        if not 0 < sample.call_begin_qpc <= sample.call_end_qpc or sample.call_begin_qpc < self.previous_call_end:
            raise RuntimeError("DXGI call timing reversed or overlaps prior attempt")
        if sample.last_present_qpc < 0 or sample.last_present_qpc > sample.call_end_qpc or sample.last_mouse_qpc < 0 or sample.last_mouse_qpc > sample.call_end_qpc:
            raise RuntimeError("DXGI source metadata contains negative or future QPC")
        intervals = {}
        for name in STAGES:
            stage = getattr(sample, name)
            if ((stage.begin == 0) != (stage.end == 0) or stage.begin < 0 or stage.end < stage.begin
                    or stage.end > sample.call_end_qpc or stage.begin and stage.begin < self.info.created_qpc):
                raise RuntimeError(f"DXGI {name} stage interval is invalid")
            intervals[name] = Interval(stage.begin, stage.end)
        active_stages = [stage for stage in intervals.values() if stage.begin]
        if any(a.end > b.begin for a, b in zip(active_stages, active_stages[1:])):
            raise RuntimeError("DXGI source stages are reversed")
        if sample.last_present_qpc and (not sample.acquire.end or sample.last_present_qpc > sample.acquire.end):
            raise RuntimeError("DXGI source timestamp is later than its acquisition")
        fingerprint = (sample.acquire.begin, sample.last_present_qpc, sample.last_mouse_qpc, sample.accumulated_frames)
        if self._held is not None and sample.acquire.begin == self._held[0] and fingerprint != self._held:
            raise RuntimeError("Resumed DXGI frame changed its original source identity")
        for name in ("protected_content_masked", "rects_coalesced", "pointer_visible", "pointer_known"):
            integer(getattr(sample, name), 0, 1, name)
        if sample.protected_content_masked:
            raise RuntimeError("DXGI desktop contains protected masked content")
        if sample.pointer_known:
            if (sample.pointer_type not in (1, 2, 4) or not 0 < sample.pointer_width <= 256
                    or not 0 < sample.pointer_height <= 512 or not 0 < sample.pointer_pitch <= 1024
                    or not 0 < sample.pointer_shape_bytes <= 524288):
                raise RuntimeError("DXGI pointer shape metadata is invalid")
        if sample.pointer_visible:
            if not sample.pointer_known:
                raise RuntimeError("DXGI visible pointer has unknown shape")
            x = self.info.output_rect[0] + sample.pointer_x
            y = self.info.output_rect[1] + sample.pointer_y
            left, top, right, bottom = self.info.roi
            if x < right and x + sample.pointer_width > left and y < bottom and y + sample.pointer_height > top:
                raise RuntimeError("DXGI pointer overlaps the meaningful display ROI")
        if status == 0:
            if (sample.pending_reason != 0 or sample.byte_count != self.region.width * self.region.height * 4
                    or not minimum <= sample.last_present_qpc or not sample.last_present_qpc
                    or not sample.accumulated_frames or sample.sequence != self.previous_sequence + 1
                    or not all(getattr(sample, name).begin for name in STAGES)
                    or sample.call_end_qpc >= deadline or any(effects[name] for name in ("poisoned", "frame_owned", "gpu_pending", "event_registered"))):
                raise RuntimeError("DXGI ready result lacks fresh complete native pixel evidence")
            pixels = bytes(sample.bgra[:sample.byte_count])
            self.previous_sequence = sample.sequence
            self._held = None
        else:
            if sample.pending_reason == 0 or sample.byte_count != 0:
                raise RuntimeError("DXGI pending result contains semantic pixel bytes")
            if sample.pending_reason == 4 and sample.last_present_qpc != 0:
                raise RuntimeError("DXGI pointer-only pending has a desktop source timestamp")
            if sample.pending_reason == 5 and not 0 < sample.last_present_qpc < minimum:
                raise RuntimeError("DXGI stale pending does not identify a stale source")
            if sample.pending_reason in (2, 3) and not effects["frame_owned"]:
                raise RuntimeError("DXGI held-frame pending lost its native ownership effect")
            if sample.pending_reason in (2, 3) and sample.release_frame.begin:
                raise RuntimeError("DXGI held-frame pending already released its source")
            pixels = None
            self._held = fingerprint if sample.pending_reason in (2, 3) else None
        self.previous_call_end = sample.call_end_qpc
        values = {name: getattr(sample, name) for name, _ in Sample._fields_
                  if name not in ("version", "size", "pending_reason", "byte_count", "call_begin_qpc", "call_end_qpc", "bgra", *STAGES)}
        values.update(output=self.info, pending_reason=PENDING[sample.pending_reason],
                      call=Interval(sample.call_begin_qpc, sample.call_end_qpc), **intervals)
        return CopyResult(self.backend, pixels, DxgiMetadata(**values))

    def close(self):
        """Condition-observe disposal within one second/256 calls; preserve ownership on failure."""
        if not self.active:
            return
        deadline = self.clock() + self.frequency
        for _ in range(256):
            error = initialized(Error)
            self.last_cleanup = {"operation": "close", "native_call_returned": False, "context_retained": self.active}
            status = self.library.close(self.context, error)
            # Native pointer mutation is authoritative even if error validation throws.
            self.last_cleanup = {"status": status, "native_call_returned": True, "context_retained": self.active,
                                 "native_error": raw_error(error)}
            status_result(status, error, "close")
            if status == 0:
                if self.active:
                    raise RuntimeError("DXGI successful close did not consume its context")
                return
            if not self.active:
                raise RuntimeError("DXGI pending close lost native resource ownership")
            now = self.clock()
            if now >= deadline:
                raise RuntimeError("DXGI cleanup deadline expired with resources still owned")
            # This yield occurs only after actual pending GPU completion evidence.
            time.sleep(min(.001, (deadline - now) / self.frequency))
        raise RuntimeError("DXGI cleanup inventory exhausted with resources still owned")
