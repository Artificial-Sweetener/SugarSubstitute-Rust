// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "observer.h"
#include <algorithm>
#include <dxgi.h>
#include <limits>

namespace ss::dxgi {
HRESULT validate_request(const ss_dxgi_request &r) noexcept {
    if (r.version != SS_DXGI_VERSION || r.size != sizeof(r) || r.width < 2 || r.width > 16 ||
        r.height < 2 || r.height > 16 || r.acquire_timeout_ms > 5 || r.fence_timeout_ms > 5 ||
        static_cast<int64_t>(r.x) + r.width > INT32_MAX ||
        static_cast<int64_t>(r.y) + r.height > INT32_MAX)
        return E_INVALIDARG;
    return S_OK;
}
Owner::Owner(Port &p, const ss_dxgi_request &r, const ss_dxgi_info &i) noexcept
    : port_(p), request_(r), info_(i), last_tick_(i.created_qpc) {}
HRESULT Owner::initialize(ss_dxgi_info &info) noexcept {
    // Durable Owner exists before any fallible native resource construction.
    const auto creator = port_.thread();
    const auto hr = port_.initialize(request_, info);
    info_ = info;
    info_.ui_thread_id = creator;
    last_tick_ = info.created_qpc;
    if (FAILED(hr))
        return fail(hr, port_.operation());
    if (info.qpc_frequency <= 0 || info.created_qpc <= 0 || info.ui_thread_id != creator ||
        creator == 0)
        return fail(E_UNEXPECTED, SS_DXGI_QPC);
    return S_OK;
}
ss_dxgi_error Owner::error() const noexcept {
    auto result = failure_;
    result.poisoned = poisoned_;
    result.frame_owned = frame_owned_;
    result.gpu_pending = port_.gpu_pending();
    result.event_registered = port_.event_registered();
    return result;
}
HRESULT Owner::fail(HRESULT hr, uint32_t op) noexcept {
    poisoned_ = true;
    if (SUCCEEDED(failure_.hresult)) {
        failure_.hresult = hr;
        failure_.operation = op;
    } else if (SUCCEEDED(failure_.cleanup_hresult) &&
               (hr != failure_.hresult || op != failure_.operation)) {
        failure_.cleanup_hresult = hr;
        failure_.cleanup_operation = op;
    }
    return failure_.hresult;
}
HRESULT Owner::stamp(int64_t &ticks) noexcept {
    const auto hr = port_.now(ticks);
    if (FAILED(hr))
        return fail(hr, SS_DXGI_QPC);
    if (ticks < last_tick_ || ticks <= 0)
        return fail(E_UNEXPECTED, SS_DXGI_QPC);
    last_tick_ = ticks;
    return S_OK;
}
HRESULT Owner::budget(int64_t deadline, uint32_t configured, uint32_t &timeout) noexcept {
    int64_t ticks = 0;
    auto hr = stamp(ticks);
    if (FAILED(hr))
        return hr;
    if (deadline <= ticks)
        return S_FALSE;
    const auto remaining = static_cast<uint64_t>(deadline - ticks);
    const auto frequency = static_cast<uint64_t>(info_.qpc_frequency);
    // Only thresholds 1..5ms are needed; divide frequency first to avoid overflow.
    const auto per_ms = frequency / 1000;
    const auto remainder = frequency % 1000;
    const auto threshold = [=](uint32_t ms) { return per_ms * ms + (remainder * ms + 999) / 1000; };
    if (remaining < threshold(1))
        return S_FALSE;
    timeout = 0;
    for (uint32_t ms = 1; ms <= configured; ++ms)
        if (remaining >= threshold(ms))
            timeout = ms;
    return S_OK;
}
HRESULT Owner::release() noexcept {
    if (!frame_owned_)
        return S_OK;
    auto clock = stamp(frame_.release_frame.begin);
    // Exactly one ReleaseFrame attempt per successful Acquire, even if it fails.
    frame_owned_ = false;
    const auto hr = port_.release_frame();
    if (FAILED(hr))
        fail(hr, SS_DXGI_RELEASE);
    const auto end = stamp(frame_.release_frame.end);
    if (FAILED(clock) || FAILED(end) || FAILED(hr))
        return failure_.hresult;
    return S_OK;
}
HRESULT Owner::end_call(ss_dxgi_sample &out, HRESULT result) noexcept {
    const auto begin = out.call_begin_qpc;
    out = frame_;
    out.version = SS_DXGI_VERSION;
    out.size = sizeof(out);
    out.call_begin_qpc = begin;
    if (result != S_OK)
        out.byte_count = 0;
    if (FAILED(stamp(out.call_end_qpc))) {
        out.byte_count = 0;
        return failure_.hresult;
    }
    return result;
}
HRESULT Owner::sample(int64_t minimum, int64_t deadline, ss_dxgi_sample &out) noexcept {
    out = {};
    out.version = SS_DXGI_VERSION;
    out.size = sizeof(out);
    if (port_.thread() != info_.ui_thread_id)
        return RPC_E_WRONG_THREAD;
    if (minimum < 0 || deadline <= 0)
        return E_INVALIDARG;
    if (poisoned_)
        return failure_.hresult;
    if (closing_)
        return E_ABORT;
    if (FAILED(stamp(out.call_begin_qpc)))
        return failure_.hresult;
    auto hr = port_.validate_output();
    if (FAILED(hr))
        return end_call(out, fail(hr, port_.operation()));
    uint32_t timeout = 0;
    hr = budget(deadline, request_.acquire_timeout_ms, timeout);
    if (hr != S_OK) {
        frame_.pending_reason = SS_DXGI_DEADLINE;
        return end_call(out, hr);
    }
    if (!frame_owned_) {
        frame_ = {};
        if (FAILED(stamp(frame_.acquire.begin)))
            return end_call(out, failure_.hresult);
        hr = port_.acquire(timeout, frame_);
        frame_owned_ = port_.frame_owned();
        if (FAILED(hr) && hr != DXGI_ERROR_WAIT_TIMEOUT)
            fail(hr, port_.operation());
        const auto end = stamp(frame_.acquire.end);
        if (FAILED(end))
            return end_call(out, failure_.hresult);
        if (hr == DXGI_ERROR_WAIT_TIMEOUT) {
            frame_.pending_reason = SS_DXGI_ACQUIRE_TIMEOUT;
            return end_call(out, S_FALSE);
        }
        if (FAILED(hr))
            return end_call(out, fail(hr, port_.operation()));
        if (!frame_owned_)
            return end_call(out, fail(E_UNEXPECTED, SS_DXGI_ACQUIRE));
        if (frame_.last_present_qpc < 0 || frame_.last_present_qpc > frame_.acquire.end ||
            frame_.last_mouse_qpc < 0 || frame_.last_mouse_qpc > frame_.acquire.end)
            return end_call(out, fail(E_UNEXPECTED, SS_DXGI_ACQUIRE));
        if (frame_.last_present_qpc == 0 || frame_.last_present_qpc < minimum) {
            frame_.pending_reason =
                frame_.last_present_qpc == 0 ? SS_DXGI_POINTER_ONLY : SS_DXGI_STALE;
            hr = release();
            return end_call(out, FAILED(hr) ? hr : S_FALSE);
        }
        if (FAILED(stamp(frame_.copy_submit.begin)))
            return end_call(out, failure_.hresult);
        hr = port_.submit_copy();
        if (FAILED(hr))
            fail(hr, port_.operation());
        const auto copied = stamp(frame_.copy_submit.end);
        if (FAILED(hr))
            return end_call(out, fail(hr, port_.operation()));
        if (FAILED(copied))
            return end_call(out, failure_.hresult);
    }
    hr = budget(deadline, request_.fence_timeout_ms, timeout);
    if (hr != S_OK) {
        frame_.pending_reason = SS_DXGI_DEADLINE;
        return end_call(out, hr);
    }
    if (FAILED(stamp(frame_.fence_wait.begin)))
        return end_call(out, failure_.hresult);
    hr = port_.wait_fence(timeout);
    if (FAILED(hr))
        fail(hr, port_.operation());
    const auto waited = stamp(frame_.fence_wait.end);
    if (FAILED(waited))
        return end_call(out, failure_.hresult);
    if (hr == S_FALSE) {
        frame_.pending_reason = SS_DXGI_FENCE_PENDING;
        return end_call(out, S_FALSE);
    }
    if (FAILED(hr))
        return end_call(out, fail(hr, port_.operation()));
    if (frame_.last_present_qpc < minimum) {
        frame_.pending_reason = SS_DXGI_STALE;
        hr = release();
        return end_call(out, FAILED(hr) ? hr : S_FALSE);
    }
    hr = budget(deadline, 0, timeout);
    if (hr != S_OK) {
        frame_.pending_reason = SS_DXGI_DEADLINE;
        return end_call(out, hr);
    }
    if (FAILED(stamp(frame_.map_read.begin)))
        return end_call(out, failure_.hresult);
    hr = port_.read_pixels(frame_);
    if (FAILED(hr) && hr != DXGI_ERROR_WAS_STILL_DRAWING)
        fail(hr, port_.operation());
    const auto read = stamp(frame_.map_read.end);
    if (FAILED(read))
        return end_call(out, failure_.hresult);
    if (hr == DXGI_ERROR_WAS_STILL_DRAWING) {
        frame_.pending_reason = SS_DXGI_MAP_PENDING;
        return end_call(out, S_FALSE);
    }
    if (FAILED(hr))
        return end_call(out, fail(hr, port_.operation()));
    if (frame_.byte_count != request_.width * request_.height * 4)
        return end_call(out, fail(E_UNEXPECTED, SS_DXGI_MAP));
    hr = port_.validate_output();
    if (FAILED(hr))
        return end_call(out, fail(hr, port_.operation()));
    hr = release();
    if (FAILED(hr))
        return end_call(out, hr);
    if (last_tick_ >= deadline) {
        frame_.pending_reason = SS_DXGI_DEADLINE;
        return end_call(out, S_FALSE);
    }
    if (sequence_ == std::numeric_limits<uint64_t>::max())
        return end_call(out, fail(E_UNEXPECTED, SS_DXGI_COPY));
    frame_.pending_reason = SS_DXGI_READY;
    hr = end_call(out, S_OK);
    if (hr == S_OK && out.call_end_qpc >= deadline) {
        out.byte_count = 0;
        out.pending_reason = SS_DXGI_DEADLINE;
        return S_FALSE;
    }
    if (hr == S_OK) {
        frame_.sequence = ++sequence_;
        out.sequence = sequence_;
    }
    return hr;
}
HRESULT Owner::close() noexcept {
    if (port_.thread() != info_.ui_thread_id)
        return RPC_E_WRONG_THREAD;
    closing_ = true;
    if (port_.gpu_pending()) {
        const auto hr = port_.wait_fence(request_.fence_timeout_ms);
        if (hr == S_FALSE)
            return S_FALSE;
        if (FAILED(hr))
            return fail(hr, port_.operation());
    }
    release();
    const auto hr = port_.dispose();
    if (FAILED(hr))
        return fail(hr, SS_DXGI_CLOSE);
    // Return success for actual cleanup while keeping all earlier causes in error().
    return S_OK;
}
} // namespace ss::dxgi
