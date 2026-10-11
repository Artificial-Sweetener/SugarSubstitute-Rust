// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "native_error.h"
#include "native_port.h"
#include "output_policy.h"
#include <cwchar>
#include <limits>

namespace ss::dxgi {
namespace {
bool equal_luid(const LUID &a, const LUID &b) noexcept {
    return a.LowPart == b.LowPart && a.HighPart == b.HighPart;
}
bool equal_rect(const RECT &a, const ss_dxgi_rect &b) noexcept {
    return a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom;
}
// Fixed caps reject unexpectedly large topology rather than allocate/retry. Resolve
// the GDI source name AND exact adapter LUID to its one active target, rejecting clones.
HRESULT advanced_color(const DXGI_OUTPUT_DESC1 &output, const LUID &luid,
                       uint32_t &flags) noexcept {
    std::array<DISPLAYCONFIG_PATH_INFO, 64> paths{};
    std::array<DISPLAYCONFIG_MODE_INFO, 128> modes{};
    UINT32 path_count = static_cast<UINT32>(paths.size()),
           mode_count = static_cast<UINT32>(modes.size());
    const auto result = QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &path_count, paths.data(),
                                           &mode_count, modes.data(), nullptr);
    if (result != ERROR_SUCCESS)
        return HRESULT_FROM_WIN32(result);
    uint32_t matches = 0;
    for (UINT32 i = 0; i < path_count; ++i) {
        const auto &path = paths[i];
        if (!equal_luid(path.sourceInfo.adapterId, luid))
            continue;
        DISPLAYCONFIG_SOURCE_DEVICE_NAME source{};
        source.header = {DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, sizeof(source),
                         path.sourceInfo.adapterId, path.sourceInfo.id};
        auto code = DisplayConfigGetDeviceInfo(&source.header);
        if (code != ERROR_SUCCESS)
            return HRESULT_FROM_WIN32(code);
        if (std::wcscmp(source.viewGdiDeviceName, output.DeviceName) != 0)
            continue;
        DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO color{};
        color.header = {DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, sizeof(color),
                        path.targetInfo.adapterId, path.targetInfo.id};
        code = DisplayConfigGetDeviceInfo(&color.header);
        if (code != ERROR_SUCCESS)
            return HRESULT_FROM_WIN32(code);
        flags = color.value;
        if (++matches != 1)
            return E_NOTIMPL;
    }
    return matches == 1 ? S_OK : E_NOTIMPL;
}
} // namespace
HRESULT NativePort::output_description(DXGI_OUTPUT_DESC1 &desc, uint32_t &flags) noexcept {
    operation_ = SS_DXGI_OUTPUT;
    auto hr = output_->GetDesc1(&desc);
    if (FAILED(hr))
        return hr;
    if (!desc.AttachedToDesktop || desc.Rotation != DXGI_MODE_ROTATION_IDENTITY || !desc.Monitor)
        return E_NOTIMPL;
    operation_ = SS_DXGI_COLOR;
    DXGI_ADAPTER_DESC1 adapter{};
    hr = adapter_->GetDesc1(&adapter);
    if (FAILED(hr))
        return hr;
    hr = advanced_color(desc, adapter.AdapterLuid, flags);
    if (FAILED(hr))
        return hr;
    return accepted_color(static_cast<uint32_t>(desc.ColorSpace), desc.BitsPerColor, flags)
               ? S_OK
               : E_NOTIMPL;
}
HRESULT NativePort::select_output(ss_dxgi_info &info) noexcept {
    operation_ = SS_DXGI_OUTPUT;
    RECT roi{request_.x, request_.y, request_.x + static_cast<LONG>(request_.width),
             request_.y + static_cast<LONG>(request_.height)};
    const auto monitor = MonitorFromRect(&roi, MONITOR_DEFAULTTONULL);
    if (!monitor)
        return E_INVALIDARG;
    ComPtr<IDXGIFactory1> factory;
    auto hr = CreateDXGIFactory1(IID_PPV_ARGS(&factory));
    if (FAILED(hr))
        return hr;
    bool found = false;
    for (UINT a = 0; a < 64 && !found; ++a) {
        ComPtr<IDXGIAdapter1> adapter;
        hr = factory->EnumAdapters1(a, &adapter);
        if (hr == DXGI_ERROR_NOT_FOUND)
            break;
        if (FAILED(hr))
            return hr;
        for (UINT o = 0; o < 64; ++o) {
            ComPtr<IDXGIOutput> output;
            hr = adapter->EnumOutputs(o, &output);
            if (hr == DXGI_ERROR_NOT_FOUND)
                break;
            if (FAILED(hr))
                return hr;
            DXGI_OUTPUT_DESC desc{};
            hr = output->GetDesc(&desc);
            if (FAILED(hr))
                return hr;
            if (desc.Monitor != monitor)
                continue;
            const auto &r = desc.DesktopCoordinates;
            if (roi.left < r.left || roi.top < r.top || roi.right > r.right ||
                roi.bottom > r.bottom)
                return E_INVALIDARG;
            adapter_ = std::move(adapter);
            hr = output.As(&output_);
            if (FAILED(hr))
                return hr;
            info.output_index = o;
            found = true;
            break;
        }
    }
    if (!found)
        return E_NOTIMPL;
    DXGI_OUTPUT_DESC1 desc{};
    uint32_t color_flags = 0;
    hr = output_description(desc, color_flags);
    info.rotation = desc.Rotation;
    info.color_space = desc.ColorSpace;
    info.bits_per_color = desc.BitsPerColor;
    info.advanced_color_flags = color_flags;
    if (FAILED(hr))
        return hr;
    MONITORINFO monitor_info{sizeof(MONITORINFO)};
    if (!GetMonitorInfoW(monitor, &monitor_info))
        return win32_failure(GetLastError());
    const auto &r = desc.DesktopCoordinates;
    if (!EqualRect(&r, &monitor_info.rcMonitor) || r.right <= r.left || r.bottom <= r.top)
        return E_NOTIMPL;
    const int64_t width = static_cast<int64_t>(r.right) - r.left;
    const int64_t height = static_cast<int64_t>(r.bottom) - r.top;
    if (width > UINT32_MAX || height > UINT32_MAX)
        return E_NOTIMPL;
    DXGI_ADAPTER_DESC1 adapter{};
    hr = adapter_->GetDesc1(&adapter);
    if (FAILED(hr))
        return hr;
    info.adapter_luid_low = adapter.AdapterLuid.LowPart;
    info.adapter_luid_high = adapter.AdapterLuid.HighPart;
    info.monitor = reinterpret_cast<uint64_t>(monitor);
    info.output_rect = {r.left, r.top, r.right, r.bottom};
    info.roi = {roi.left, roi.top, roi.right, roi.bottom};
    info.rotation = desc.Rotation;
    info.color_space = desc.ColorSpace;
    info.bits_per_color = desc.BitsPerColor;
    info.advanced_color_flags = color_flags;
    info.desktop_width = static_cast<uint32_t>(width);
    info.desktop_height = static_cast<uint32_t>(height);
    memcpy(info.output_name, desc.DeviceName, sizeof(info.output_name));
    memcpy(info.adapter_description, adapter.Description, sizeof(info.adapter_description));
    return S_OK;
}
HRESULT NativePort::validate_output() noexcept {
    operation_ = SS_DXGI_THREAD;
    if (!AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),
                                      DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
        return E_NOTIMPL;
    DXGI_OUTPUT_DESC1 desc{};
    uint32_t flags = 0;
    auto hr = output_description(desc, flags);
    if (FAILED(hr))
        return hr;
    operation_ = SS_DXGI_OUTPUT;
    if (reinterpret_cast<uint64_t>(desc.Monitor) != info_.monitor ||
        !equal_rect(desc.DesktopCoordinates, info_.output_rect) ||
        static_cast<uint32_t>(desc.Rotation) != info_.rotation ||
        static_cast<uint32_t>(desc.ColorSpace) != info_.color_space ||
        desc.BitsPerColor != info_.bits_per_color || flags != info_.advanced_color_flags ||
        memcmp(desc.DeviceName, info_.output_name, sizeof(info_.output_name)) != 0)
        return E_ABORT;
    return check_device();
}
} // namespace ss::dxgi
