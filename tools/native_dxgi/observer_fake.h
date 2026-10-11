// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include "observer_test_support.h"
#include <cstring>
namespace ss::dxgi::tests {
struct Fake final : Port {
    HRESULT acquisition = S_OK, copy = S_OK, fence = S_OK, map = S_OK;
    HRESULT release = S_OK, disposal = S_OK, output = S_OK, construction = S_OK;
    int now_calls = 0, fail_now = 0, acquire_calls = 0, release_calls = 0, dispose_calls = 0;
    uint32_t acquire_timeout = 99, fence_timeout = 99;
    int64_t ticks = 10, present = 10, created = 10;
    DWORD current_thread = 7;
    uint32_t current_operation = SS_DXGI_COPY;
    bool acquired = false, pending = false, registration = false;
    bool partial_acquire = false;
    HRESULT initialize(const ss_dxgi_request &, ss_dxgi_info &out) noexcept override {
        if (construction == S_OK) {
            out.ui_thread_id = current_thread;
            out.qpc_frequency = 1000;
            out.created_qpc = created;
        }
        return construction;
    }
    DWORD thread() const noexcept override { return current_thread; }
    HRESULT now(int64_t &value) noexcept override {
        ++now_calls;
        if (now_calls == fail_now)
            return E_FAIL;
        value = ticks++;
        return S_OK;
    }
    HRESULT validate_output() noexcept override { return output; }
    HRESULT acquire(uint32_t timeout, ss_dxgi_sample &sample) noexcept override {
        ++acquire_calls;
        acquire_timeout = timeout;
        acquired = acquisition == S_OK || partial_acquire;
        sample.last_present_qpc = present;
        sample.accumulated_frames = present ? 1u : 0u;
        return acquisition;
    }
    HRESULT submit_copy() noexcept override {
        pending = true;
        registration = true;
        return copy;
    }
    HRESULT wait_fence(uint32_t timeout) noexcept override {
        fence_timeout = timeout;
        if (fence == S_OK) {
            pending = false;
            registration = false;
        }
        return fence;
    }
    HRESULT read_pixels(ss_dxgi_sample &sample) noexcept override {
        if (map == S_OK) {
            sample.byte_count = 16;
            std::memset(sample.bgra, 0x71, 16);
        }
        return map;
    }
    HRESULT release_frame() noexcept override {
        ++release_calls;
        acquired = false;
        return release;
    }
    HRESULT dispose() noexcept override {
        ++dispose_calls;
        return disposal;
    }
    bool gpu_pending() const noexcept override { return pending; }
    bool frame_owned() const noexcept override { return acquired; }
    bool event_registered() const noexcept override { return registration; }
    uint32_t operation() const noexcept override { return current_operation; }
};
inline ss_dxgi_info info() {
    ss_dxgi_info result{};
    result.ui_thread_id = 7;
    result.qpc_frequency = 1000;
    result.created_qpc = 10;
    return result;
}
} // namespace ss::dxgi::tests
