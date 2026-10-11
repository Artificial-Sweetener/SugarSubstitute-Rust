// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include <stddef.h>
#include <stdint.h>

// Windows x64 cdecl ABI, default 8-byte packing. All input/output pointers are
// caller-owned valid storage; contexts are private, noncopyable and creating-thread
// affine. No function starts a thread, loop, timer or child process. The caller owns
// HWND/process/visibility/geometry/security witnesses around the complete call.
// Finite DXGI/event timeouts do not guarantee a wall-clock bound for driver calls.
#ifdef SS_DXGI_BUILD
#define SS_DXGI_API __declspec(dllexport)
#else
#define SS_DXGI_API __declspec(dllimport)
#endif
#ifdef __cplusplus
extern "C" {
#endif
#pragma pack(push, 8)
enum { SS_DXGI_VERSION = 1, SS_DXGI_MAX_BYTES = 1024 };
typedef struct ss_dxgi_context ss_dxgi_context;
typedef struct ss_dxgi_rect {
    int32_t left, top, right, bottom;
} ss_dxgi_rect;
typedef struct ss_dxgi_interval {
    int64_t begin, end;
} ss_dxgi_interval;
enum ss_dxgi_pending {
    SS_DXGI_READY = 0,
    SS_DXGI_ACQUIRE_TIMEOUT = 1,
    SS_DXGI_FENCE_PENDING = 2,
    SS_DXGI_MAP_PENDING = 3,
    SS_DXGI_POINTER_ONLY = 4,
    SS_DXGI_STALE = 5,
    SS_DXGI_DEADLINE = 6
};
enum ss_dxgi_operation {
    SS_DXGI_NONE = 0,
    SS_DXGI_ARGUMENT = 1,
    SS_DXGI_THREAD = 2,
    SS_DXGI_QPC = 3,
    SS_DXGI_OUTPUT = 4,
    SS_DXGI_COLOR = 5,
    SS_DXGI_DEVICE = 6,
    SS_DXGI_STAGING = 7,
    SS_DXGI_FENCE = 8,
    SS_DXGI_ACQUIRE = 9,
    SS_DXGI_POINTER = 10,
    SS_DXGI_COPY = 11,
    SS_DXGI_MAP = 12,
    SS_DXGI_RELEASE = 13,
    SS_DXGI_CLOSE = 14
};
// Original HRESULT and first independent cleanup failure are preserved numerically.
// Poisoned contexts accept close only; close may remain pending while GPU work owns
// its fence/event/frame. Never free a context pointer or unload the DLL before close
// consumes it. S_FALSE keeps ownership and is never a pixel/semantic success.
typedef struct ss_dxgi_error {
    uint32_t version, size;
    int32_t hresult;
    uint32_t operation;
    int32_t cleanup_hresult;
    uint32_t cleanup_operation;
    uint32_t poisoned, frame_owned, gpu_pending, event_registered;
} ss_dxgi_error;
typedef struct ss_dxgi_request {
    uint32_t version, size;
    int32_t x, y;           // Physical GLOBAL desktop origin; negative origins are supported.
    uint32_t width, height; // Each 2..16 physical pixels, wholly in one output.
    uint32_t acquire_timeout_ms, fence_timeout_ms; // Each 0..5; never INFINITE.
} ss_dxgi_request;
typedef struct ss_dxgi_info {
    uint32_t version, size;
    uint32_t ui_thread_id, output_index;
    int64_t qpc_frequency, created_qpc;
    uint64_t monitor; // Informational opaque HMONITOR, no ownership transfer.
    uint32_t adapter_luid_low;
    int32_t adapter_luid_high;
    ss_dxgi_rect output_rect, roi; // Physical GLOBAL desktop coordinates.
    uint32_t rotation, color_space, bits_per_color, advanced_color_flags;
    // DisplayConfig bitmask: SDR accepts CS0/BPC8 with enabled(2)/wide(4) clear;
    // prospective OS-converted HDR accepts ONLY CS12/BPC10/flags2 or3. Capability
    // supported(bit1 value1) is informational; enabled/wide/disabled stay checked.
    // Unknown modes
    // reject. HDR gamut fidelity is not claimed; actual BGRA/GDI parity is required.
    uint32_t texture_format, desktop_width, desktop_height;
    // Required BGRA8 source/staging format87; validated on EVERY acquired texture.
    // Output mode descriptor may report FP16, and is not its acquired resource.
    uint32_t staging_width, staging_height;
    uint16_t output_name[32], adapter_description[128];
} ss_dxgi_info;
typedef struct ss_dxgi_sample {
    uint32_t version, size;
    uint32_t pending_reason, byte_count;
    int64_t call_begin_qpc, call_end_qpc, last_present_qpc, last_mouse_qpc;
    uint64_t sequence; // Successful-ready ordinal, not desktop frame/present count.
    uint32_t accumulated_frames, protected_content_masked, rects_coalesced;
    uint32_t pointer_visible, pointer_known, pointer_type;
    int32_t pointer_x, pointer_y; // Output-relative shape top-left, not hotspot.
    uint32_t pointer_width, pointer_height, pointer_pitch, pointer_shape_bytes;
    // pointer_height is visible height; monochrome double-mask height is divided by2.
    ss_dxgi_interval acquire, copy_submit, fence_wait, map_read, release_frame;
    uint8_t bgra[SS_DXGI_MAX_BYTES]; // Tight rows, no cursor synthesis/color conversion.
} ss_dxgi_sample;
#pragma pack(pop)

// Validate version/size and PMv2 before selecting the containing real HMONITOR and
// its adapter. Reject rotation, unknown color mode, unsafe geometry/unsupported device.
// Callers initialize version=1 and size=sizeof for EVERY POD, and *context=null.
// Failed creation normally leaves *context=null after actual cleanup. If partial
// cleanup fails it returns FAILED HRESULT with a NONNULL poisoned close-only owner.
// Preserve that pointer/DLL lifetime for explicit cleanup. No frame is acquired.
SS_DXGI_API int32_t __cdecl ss_dxgi_v1_create(const ss_dxgi_request *request,
                                              ss_dxgi_context **context, ss_dxgi_info *info,
                                              ss_dxgi_error *error);
// Acquire at most once, copy only the enrolled ROI, poll/wait the GPU fence once and
// Map(READ, DO_NOT_WAIT) once. A held frame resumes on later calls, never reacquires.
// minimum_present_qpc=0 accepts any NONZERO source update; positive values enforce
// the original input-begin floor. deadline_qpc must be a positive absolute QPC;
// baseline callers also enroll their original deadline. Each
// finite wait is clamped to remaining whole milliseconds. Caller still checks its
// original absolute deadline after all native/ownership guards. Pending returns no
// bytes; LastPresentTime=0 pointer-only updates never become semantic observations.
// Phase intervals may belong to an earlier call while its same frame remains held;
// zero means that stage did not start, not a fabricated instantaneous measurement.
// Source update QPC can precede enrollment (baseline min0), unlike helper phases.
// minimum>deadline cannot produce fresh feedback and eventually returns Deadline.
SS_DXGI_API int32_t __cdecl ss_dxgi_v1_try_sample(ss_dxgi_context *context,
                                                  int64_t minimum_present_qpc, int64_t deadline_qpc,
                                                  ss_dxgi_sample *sample, ss_dxgi_error *error);
// One finite fence wait, then exactly-once frame release and ordered COM/event cleanup.
// S_OK consumes/zeros *context. S_FALSE or failed HRESULT retains ownership for close
// retry; primary failure never disappears from error. S_OK means cleanup consumed
// ownership, while error can still contain an earlier cohort failure to report.
// Null context close is idempotent S_OK. First cleanup cause is retained, not an
// unlimited history. A failed Signal has no GPU-stop proof; keep its poisoned owner
// for root-controlled process containment if explicit close cannot ever settle.
SS_DXGI_API int32_t __cdecl ss_dxgi_v1_close(ss_dxgi_context **context, ss_dxgi_error *error);
#ifdef __cplusplus
}
static_assert(sizeof(ss_dxgi_request) == 32);
static_assert(sizeof(ss_dxgi_error) == 40);
static_assert(sizeof(ss_dxgi_info) == 440);
static_assert(sizeof(ss_dxgi_sample) == 1208);
static_assert(offsetof(ss_dxgi_sample, bgra) == 184);
static_assert(alignof(ss_dxgi_info) == 8 && alignof(ss_dxgi_sample) == 8);
#endif
