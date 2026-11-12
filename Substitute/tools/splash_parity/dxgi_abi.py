# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Exact cdecl x64 native_dxgi.h v1 ABI and verified, restricted DLL loading."""
from __future__ import annotations
import ctypes as c
import hashlib
import os
import re
from dataclasses import dataclass
from pathlib import Path

U, I, Q, L = c.c_uint32, c.c_int32, c.c_uint64, c.c_int64


class Rect(c.Structure):
    _pack_ = 8
    _fields_ = [(name, I) for name in ("left", "top", "right", "bottom")]


class NativeInterval(c.Structure):
    _pack_ = 8
    _fields_ = [("begin", L), ("end", L)]


class Request(c.Structure):
    _pack_ = 8
    _fields_ = [("version", U), ("size", U), ("x", I), ("y", I),
                ("width", U), ("height", U), ("acquire_timeout_ms", U), ("fence_timeout_ms", U)]


class Error(c.Structure):
    _pack_ = 8
    _fields_ = [("version", U), ("size", U), ("hresult", I), ("operation", U),
                ("cleanup_hresult", I), ("cleanup_operation", U),
                ("poisoned", U), ("frame_owned", U), ("gpu_pending", U), ("event_registered", U)]


class Info(c.Structure):
    _pack_ = 8
    _fields_ = [("version", U), ("size", U), ("ui_thread_id", U), ("output_index", U),
                ("qpc_frequency", L), ("created_qpc", L), ("monitor", Q),
                ("adapter_luid_low", U), ("adapter_luid_high", I), ("output_rect", Rect), ("roi", Rect),
                ("rotation", U), ("color_space", U), ("bits_per_color", U), ("advanced_color_flags", U),
                ("texture_format", U), ("desktop_width", U), ("desktop_height", U),
                ("staging_width", U), ("staging_height", U),
                ("output_name", c.c_uint16 * 32), ("adapter_description", c.c_uint16 * 128)]


class Sample(c.Structure):
    _pack_ = 8
    _fields_ = [("version", U), ("size", U), ("pending_reason", U), ("byte_count", U),
                ("call_begin_qpc", L), ("call_end_qpc", L), ("last_present_qpc", L), ("last_mouse_qpc", L),
                ("sequence", Q), ("accumulated_frames", U), ("protected_content_masked", U),
                ("rects_coalesced", U), ("pointer_visible", U), ("pointer_known", U), ("pointer_type", U),
                ("pointer_x", I), ("pointer_y", I), ("pointer_width", U), ("pointer_height", U),
                ("pointer_pitch", U), ("pointer_shape_bytes", U)] + [
                    (name, NativeInterval) for name in ("acquire", "copy_submit", "fence_wait", "map_read", "release_frame")
                ] + [("bgra", c.c_uint8 * 1024)]


def initialized(kind):
    value = kind()
    value.version, value.size = 1, c.sizeof(kind)
    return value


def validate_layout():
    for kind, size in ((Request, 32), (Error, 40), (Info, 440), (Sample, 1208)):
        if c.sizeof(kind) != size:
            raise RuntimeError(f"Unexpected {kind.__name__} native ABI size")
    if c.sizeof(c.c_void_p) != 8 or c.alignment(Info) != 8 or c.alignment(Sample) != 8 or Sample.bgra.offset != 184:
        raise RuntimeError("Native DXGI requires exact x64 eight-byte ABI packing")


@dataclass(frozen=True)
class CaptureHelper:
    """Pre-enrolled absolute helper identity; validate before launching the product."""
    path: Path
    sha256: str

    @classmethod
    def enroll(cls, path, digest):
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-fA-F]{64}", digest) is None:
            raise ValueError("Capture helper SHA256 must be exactly64 hexadecimal characters")
        supplied = Path(path)
        if not supplied.is_absolute() or supplied.suffix.casefold() != ".dll":
            raise ValueError("Capture helper must be an absolute DLL path")
        resolved = supplied.resolve(strict=True)
        if not resolved.is_file() or resolved.stat().st_size > 32 * 1024 * 1024:
            raise ValueError("Capture helper is not a bounded DLL file")
        actual = hashlib.sha256(resolved.read_bytes()).hexdigest()
        if actual != digest.lower():
            raise ValueError("Capture helper SHA256 differs from the explicit cohort identity")
        return cls(resolved, actual)

    def verify(self):
        if CaptureHelper.enroll(self.path, self.sha256) != self:
            raise RuntimeError("Capture helper identity changed")


def enroll_helper(path, digest):
    if (path is None) != (digest is None):
        raise ValueError("--capture-helper and --capture-helper-sha256 must be supplied together")
    return None if path is None else CaptureHelper.enroll(path, digest)


class DxgiLibrary:
    """Load only the exact enrolled DLL with DLL-dir/System32 search, never PATH.

    Loading occurs before product launch; no device/context is created here.
    Keep this object alive until every native context is actually consumed.
    """
    def __init__(self, helper):
        validate_layout()
        if os.name != "nt":
            raise OSError("Native DXGI helper loading requires Windows")
        helper.verify()
        self.helper = helper
        self.dll = c.CDLL(str(helper.path), use_last_error=True, winmode=0x100 | 0x800)
        helper.verify()
        pointer, context = c.POINTER, c.c_void_p
        for name, args in (
            ("ss_dxgi_v1_create", [pointer(Request), pointer(context), pointer(Info), pointer(Error)]),
            ("ss_dxgi_v1_try_sample", [context, L, L, pointer(Sample), pointer(Error)]),
            ("ss_dxgi_v1_close", [pointer(context), pointer(Error)])):
            function = getattr(self.dll, name)
            function.argtypes, function.restype = args, I

    def create(self, request, context, info, error):
        return self.dll.ss_dxgi_v1_create(c.byref(request), c.byref(context), c.byref(info), c.byref(error))

    def attempt(self, context, minimum, deadline, sample, error):
        return self.dll.ss_dxgi_v1_try_sample(context, minimum, deadline, c.byref(sample), c.byref(error))

    def close(self, context, error):
        return self.dll.ss_dxgi_v1_close(c.byref(context), c.byref(error))
