// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include "native_dxgi.h"
#include <Windows.h>
#include <memory>

namespace ss::dxgi {
// External native calls only: the same Owner controls actual and injected ports.
// No port may implement retries, ownership decisions or synthesized pixel samples.
struct Port {
    virtual ~Port() = default;
    virtual HRESULT initialize(const ss_dxgi_request &, ss_dxgi_info &) noexcept = 0;
    virtual DWORD thread() const noexcept = 0;
    virtual HRESULT now(int64_t &) noexcept = 0;
    virtual HRESULT validate_output() noexcept = 0;
    virtual HRESULT acquire(uint32_t, ss_dxgi_sample &) noexcept = 0;
    virtual HRESULT submit_copy() noexcept = 0;
    virtual HRESULT wait_fence(uint32_t) noexcept = 0;
    virtual HRESULT read_pixels(ss_dxgi_sample &) noexcept = 0;
    virtual HRESULT release_frame() noexcept = 0;
    virtual HRESULT dispose() noexcept = 0;
    virtual bool gpu_pending() const noexcept = 0;
    virtual bool frame_owned() const noexcept = 0;
    virtual bool event_registered() const noexcept = 0;
    virtual uint32_t operation() const noexcept = 0;
};

// Retain the original error and every acquired-frame obligation through failed
// counter reads. Only explicit close, after finite GPU settlement, permits deletion.
class Owner final {
  public:
    Owner(Port &port, const ss_dxgi_request &request, const ss_dxgi_info &info) noexcept;
    HRESULT initialize(ss_dxgi_info &info) noexcept;
    HRESULT sample(int64_t minimum, int64_t deadline, ss_dxgi_sample &output) noexcept;
    HRESULT close() noexcept;
    ss_dxgi_error error() const noexcept;

  private:
    HRESULT fail(HRESULT result, uint32_t operation) noexcept;
    HRESULT stamp(int64_t &ticks) noexcept;
    HRESULT budget(int64_t deadline, uint32_t configured, uint32_t &timeout) noexcept;
    HRESULT release() noexcept;
    HRESULT end_call(ss_dxgi_sample &output, HRESULT result) noexcept;
    Port &port_;
    ss_dxgi_request request_;
    ss_dxgi_info info_;
    ss_dxgi_sample frame_{};
    ss_dxgi_error failure_{SS_DXGI_VERSION, sizeof(ss_dxgi_error)};
    int64_t last_tick_;
    uint64_t sequence_ = 0;
    bool frame_owned_ = false;
    bool poisoned_ = false;
    bool closing_ = false;
};
std::unique_ptr<Port> native_port();
HRESULT validate_request(const ss_dxgi_request &request) noexcept;
} // namespace ss::dxgi
