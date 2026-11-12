// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "native_error.h"
#include "native_port.h"
#include "pixel_rows.h"
#include <cstdio>
#include <cstring>
#include <limits>

namespace ss::dxgi {
HRESULT NativePort::now(int64_t &value) noexcept {
    LARGE_INTEGER ticks{};
    if (!QueryPerformanceCounter(&ticks))
        return win32_failure(GetLastError());
    if (ticks.QuadPart <= 0)
        return E_UNEXPECTED;
    value = ticks.QuadPart;
    return S_OK;
}
HRESULT NativePort::check_device() noexcept {
    operation_ = SS_DXGI_DEVICE;
    return device_->GetDeviceRemovedReason();
}
HRESULT NativePort::initialize(const ss_dxgi_request &request, ss_dxgi_info &info) noexcept {
    request_ = request;
    info = {};
    info.version = SS_DXGI_VERSION;
    info.size = sizeof(info);
    operation_ = SS_DXGI_THREAD;
    if (!AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),
                                      DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
        return E_NOTIMPL;
    info.ui_thread_id = thread();
    operation_ = SS_DXGI_QPC;
    LARGE_INTEGER frequency{};
    if (!QueryPerformanceFrequency(&frequency))
        return win32_failure(GetLastError());
    if (frequency.QuadPart <= 0)
        return E_UNEXPECTED;
    info.qpc_frequency = frequency.QuadPart;
    auto hr = now(info.created_qpc);
    if (FAILED(hr))
        return hr;
    hr = select_output(info);
    if (FAILED(hr))
        return hr;
    operation_ = SS_DXGI_DEVICE;
    constexpr D3D_FEATURE_LEVEL levels[]{D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0};
    hr = D3D11CreateDevice(adapter_.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
                           D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                           levels, static_cast<UINT>(std::size(levels)), D3D11_SDK_VERSION,
                           &device_, nullptr, &immediate_);
    if (FAILED(hr))
        return hr;
    if (FAILED(hr = device_.As(&device5_)))
        return hr;
    if (FAILED(hr = immediate_.As(&immediate4_)))
        return hr;
    operation_ = SS_DXGI_STAGING;
    D3D11_TEXTURE2D_DESC texture{};
    texture.Width = request.width;
    texture.Height = request.height;
    texture.MipLevels = 1;
    texture.ArraySize = 1;
    texture.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    texture.SampleDesc.Count = 1;
    texture.Usage = D3D11_USAGE_STAGING;
    texture.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    hr = device_->CreateTexture2D(&texture, nullptr, &staging_);
    if (FAILED(hr))
        return hr;
    info.texture_format = texture.Format;
    info.staging_width = texture.Width;
    info.staging_height = texture.Height;
    operation_ = SS_DXGI_FENCE;
    hr = device5_->CreateFence(0, D3D11_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence_));
    if (FAILED(hr))
        return hr;
    event_ = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (!event_)
        return win32_failure(GetLastError());
    operation_ = SS_DXGI_OUTPUT;
    ComPtr<IDXGIOutput1> output1;
    hr = output_.As(&output1);
    if (FAILED(hr))
        return hr;
    hr = output1->DuplicateOutput(device_.Get(), &duplication_);
    if (FAILED(hr))
        return hr;
    DXGI_OUTDUPL_DESC desc{};
    duplication_->GetDesc(&desc);
    // ModeDesc describes the output mode (FP16 on HDR), not proof of acquired texture
    // format. Original DuplicateOutput's actual resource is checked before every copy.
    if (desc.ModeDesc.Width != info.desktop_width || desc.ModeDesc.Height != info.desktop_height ||
        desc.Rotation != DXGI_MODE_ROTATION_IDENTITY)
        return E_NOTIMPL;
    info_ = info;
    return S_OK;
}
HRESULT NativePort::update_pointer(const DXGI_OUTDUPL_FRAME_INFO &info,
                                   ss_dxgi_sample &sample) noexcept {
    operation_ = SS_DXGI_POINTER;
    if (info.LastMouseUpdateTime.QuadPart != 0) {
        pointer_visible_ = info.PointerPosition.Visible != FALSE;
        pointer_position_ = info.PointerPosition.Position;
    }
    if (info.PointerShapeBufferSize != 0) {
        if (info.PointerShapeBufferSize > shape_storage_.size())
            return E_NOTIMPL;
        UINT needed = 0;
        auto hr =
            duplication_->GetFramePointerShape(static_cast<UINT>(shape_storage_.size()),
                                               shape_storage_.data(), &needed, &pointer_shape_);
        if (FAILED(hr))
            return hr;
        if (needed > shape_storage_.size() || pointer_shape_.Width == 0 ||
            pointer_shape_.Width > 256 || pointer_shape_.Height == 0 ||
            pointer_shape_.Height > 512 ||
            (pointer_shape_.Type != DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR &&
             pointer_shape_.Type != DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME &&
             pointer_shape_.Type != DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR))
            return E_NOTIMPL;
        if (pointer_shape_.Type == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME &&
            pointer_shape_.Height % 2 != 0)
            return E_NOTIMPL;
        shape_bytes_ = needed;
        pointer_known_ = true;
    }
    sample.pointer_visible = pointer_visible_;
    sample.pointer_known = pointer_known_;
    sample.pointer_type = pointer_shape_.Type;
    sample.pointer_x = pointer_position_.x;
    sample.pointer_y = pointer_position_.y;
    sample.pointer_width = pointer_shape_.Width;
    sample.pointer_height = pointer_shape_.Type == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME
                                ? pointer_shape_.Height / 2
                                : pointer_shape_.Height;
    sample.pointer_pitch = pointer_shape_.Pitch;
    sample.pointer_shape_bytes = shape_bytes_;
    if (!pointer_visible_)
        return S_OK;
    if (!pointer_known_)
        return E_NOTIMPL;
    const int64_t left = static_cast<int64_t>(info_.output_rect.left) + pointer_position_.x;
    const int64_t top = static_cast<int64_t>(info_.output_rect.top) + pointer_position_.y;
    if (left < info_.roi.right && left + sample.pointer_width > info_.roi.left &&
        top < info_.roi.bottom && top + sample.pointer_height > info_.roi.top)
        return E_ACCESSDENIED;
    return S_OK;
}
HRESULT NativePort::acquire(uint32_t timeout, ss_dxgi_sample &sample) noexcept {
    operation_ = SS_DXGI_ACQUIRE;
    DXGI_OUTDUPL_FRAME_INFO frame{};
    const auto hr = duplication_->AcquireNextFrame(timeout, &frame, &frame_resource_);
    if (hr != S_OK)
        return hr;
    acquired_ = true;
    sample.last_present_qpc = frame.LastPresentTime.QuadPart;
    sample.last_mouse_qpc = frame.LastMouseUpdateTime.QuadPart;
    sample.accumulated_frames = frame.AccumulatedFrames;
    sample.protected_content_masked = frame.ProtectedContentMaskedOut != FALSE;
    sample.rects_coalesced = frame.RectsCoalesced != FALSE;
    if (frame.ProtectedContentMaskedOut)
        return E_ACCESSDENIED;
    return update_pointer(frame, sample);
}
HRESULT NativePort::submit_copy() noexcept {
    operation_ = SS_DXGI_COPY;
    ComPtr<ID3D11Texture2D> texture;
    auto hr = frame_resource_.As(&texture);
    if (FAILED(hr))
        return hr;
    D3D11_TEXTURE2D_DESC desc{};
    texture->GetDesc(&desc);
    if (desc.Width != info_.desktop_width || desc.Height != info_.desktop_height ||
        desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM || desc.ArraySize != 1 ||
        desc.SampleDesc.Count != 1 || desc.MipLevels != 1)
        return E_NOTIMPL;
    if (fence_value_ == std::numeric_limits<uint64_t>::max())
        return E_UNEXPECTED;
    const auto x = static_cast<UINT>(static_cast<int64_t>(request_.x) - info_.output_rect.left);
    const auto y = static_cast<UINT>(static_cast<int64_t>(request_.y) - info_.output_rect.top);
    const D3D11_BOX box{x, y, 0, x + request_.width, y + request_.height, 1};
    if (!ResetEvent(event_))
        return win32_failure(GetLastError());
    immediate_->CopySubresourceRegion(staging_.Get(), 0, 0, 0, 0, texture.Get(), 0, &box);
    pending_ = true;
    operation_ = SS_DXGI_FENCE;
    hr = immediate4_->Signal(fence_.Get(), ++fence_value_);
    if (FAILED(hr)) {
        copy_signal_failure_ = hr;
        return hr;
    }
    hr = fence_->SetEventOnCompletion(fence_value_, event_);
    if (SUCCEEDED(hr))
        registered_ = true;
    immediate_->Flush();
    return hr;
}
HRESULT NativePort::wait_fence(uint32_t timeout) noexcept {
    operation_ = SS_DXGI_FENCE;
    if (!pending_)
        return S_OK;
    // Failed Signal has no completion proof: keep the caller's poisoned context owned.
    if (FAILED(copy_signal_failure_))
        return copy_signal_failure_;
    auto hr = check_device();
    if (FAILED(hr))
        return hr;
    operation_ = SS_DXGI_FENCE;
    auto completed = fence_->GetCompletedValue();
    if (completed == std::numeric_limits<uint64_t>::max()) {
        hr = check_device();
        return FAILED(hr) ? hr : E_UNEXPECTED;
    }
    if (completed >= fence_value_) {
        pending_ = false;
        registered_ = false;
        return S_OK;
    }
    if (!registered_)
        return S_FALSE;
    const auto status = WaitForSingleObject(event_, timeout);
    if (status == WAIT_TIMEOUT)
        return S_FALSE;
    if (status == WAIT_FAILED)
        return win32_failure(GetLastError());
    if (status != WAIT_OBJECT_0)
        return E_UNEXPECTED;
    completed = fence_->GetCompletedValue();
    if (completed == std::numeric_limits<uint64_t>::max()) {
        hr = check_device();
        return FAILED(hr) ? hr : E_UNEXPECTED;
    }
    if (completed < fence_value_)
        return E_UNEXPECTED;
    pending_ = false;
    registered_ = false;
    return S_OK;
}
HRESULT NativePort::read_pixels(ss_dxgi_sample &sample) noexcept {
    operation_ = SS_DXGI_MAP;
    D3D11_MAPPED_SUBRESOURCE mapped{};
    auto hr =
        immediate_->Map(staging_.Get(), 0, D3D11_MAP_READ, D3D11_MAP_FLAG_DO_NOT_WAIT, &mapped);
    if (hr != S_OK)
        return hr;
    hr = copy_rows(mapped.pData, mapped.RowPitch, request_.width, request_.height, sample);
    immediate_->Unmap(staging_.Get(), 0);
    return hr;
}
HRESULT NativePort::release_frame() noexcept {
    operation_ = SS_DXGI_RELEASE;
    if (!acquired_)
        return DXGI_ERROR_INVALID_CALL;
    acquired_ = false;
    const auto hr = duplication_->ReleaseFrame();
    frame_resource_.Reset();
    return hr;
}
HRESULT NativePort::dispose() noexcept {
    if (pending_ || acquired_ || registered_)
        return E_PENDING;
    frame_resource_.Reset();
    duplication_.Reset();
    staging_.Reset();
    fence_.Reset();
    immediate4_.Reset();
    immediate_.Reset();
    device5_.Reset();
    device_.Reset();
    output_.Reset();
    adapter_.Reset();
    if (event_) {
        if (!CloseHandle(event_))
            return win32_failure(GetLastError());
        event_ = nullptr;
    }
    return S_OK;
}
NativePort::~NativePort() {
    // Construction failures own no acquired frame or fence registration. Fully
    // constructed owners are destroyed only after explicit dispose succeeds.
    if (event_ && !CloseHandle(event_))
        std::fprintf(stderr, "dxgi construction event cleanup failed: %lu\n", GetLastError());
}
std::unique_ptr<Port> native_port() { return std::make_unique<NativePort>(); }
} // namespace ss::dxgi
