// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include "observer.h"
#include <array>
#include <d3d11_4.h>
#include <dxgi1_6.h>
#include <wrl/client.h>

namespace ss::dxgi {
using Microsoft::WRL::ComPtr;
// COM references are released in dependency order by dispose, only after the fence
// has settled. The fixed event remains owned through any pending GPU registration.
class NativePort final : public Port {
  public:
    ~NativePort() override;
    HRESULT initialize(const ss_dxgi_request &, ss_dxgi_info &) noexcept override;
    DWORD thread() const noexcept override { return GetCurrentThreadId(); }
    HRESULT now(int64_t &value) noexcept override;
    HRESULT validate_output() noexcept override;
    HRESULT acquire(uint32_t, ss_dxgi_sample &) noexcept override;
    HRESULT submit_copy() noexcept override;
    HRESULT wait_fence(uint32_t) noexcept override;
    HRESULT read_pixels(ss_dxgi_sample &) noexcept override;
    HRESULT release_frame() noexcept override;
    HRESULT dispose() noexcept override;
    bool gpu_pending() const noexcept override { return pending_; }
    bool frame_owned() const noexcept override { return acquired_; }
    bool event_registered() const noexcept override { return registered_; }
    uint32_t operation() const noexcept override { return operation_; }

  private:
    HRESULT select_output(ss_dxgi_info &) noexcept;
    HRESULT output_description(DXGI_OUTPUT_DESC1 &, uint32_t &) noexcept;
    HRESULT update_pointer(const DXGI_OUTDUPL_FRAME_INFO &, ss_dxgi_sample &) noexcept;
    HRESULT check_device() noexcept;
    ss_dxgi_request request_{};
    ss_dxgi_info info_{};
    ComPtr<IDXGIAdapter1> adapter_;
    ComPtr<IDXGIOutput6> output_;
    ComPtr<ID3D11Device> device_;
    ComPtr<ID3D11Device5> device5_;
    ComPtr<ID3D11DeviceContext> immediate_;
    ComPtr<ID3D11DeviceContext4> immediate4_;
    ComPtr<ID3D11Texture2D> staging_;
    ComPtr<ID3D11Fence> fence_;
    ComPtr<IDXGIOutputDuplication> duplication_;
    ComPtr<IDXGIResource> frame_resource_;
    HANDLE event_ = nullptr;
    uint64_t fence_value_ = 0;
    uint32_t operation_ = SS_DXGI_NONE;
    bool acquired_ = false, pending_ = false, registered_ = false;
    bool pointer_known_ = false, pointer_visible_ = false;
    POINT pointer_position_{};
    DXGI_OUTDUPL_POINTER_SHAPE_INFO pointer_shape_{};
    uint32_t shape_bytes_ = 0;
    HRESULT copy_signal_failure_ = S_OK;
    std::array<uint8_t, 65536> shape_storage_{};
};
} // namespace ss::dxgi
