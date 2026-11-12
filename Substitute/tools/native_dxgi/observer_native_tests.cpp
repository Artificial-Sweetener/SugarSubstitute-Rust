// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Noncapture native-resource and exact pixel-row qualification owner.
#include "observer_test_support.h"
#include "output_policy.h"
#include "pixel_rows.h"
#include <array>
#include <cstdio>
#include <d3d11_4.h>
#include <dxgi1_6.h>
#include <wrl/client.h>
namespace {
using namespace ss::dxgi;
using namespace ss::dxgi::tests;
void native_enrollment_and_release_without_capture() {
    const auto previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    check(previous != nullptr, "PMv2 native test admission failed");
    auto p = native_port();
    auto r = request();
    ss_dxgi_info i{};
    MONITORINFO monitor{sizeof(MONITORINFO)};
    const POINT origin{};
    check(GetMonitorInfoW(MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY), &monitor) != FALSE,
          "Native monitor query failed");
    r.x = monitor.rcMonitor.left + 2;
    r.y = monitor.rcMonitor.top + 2;
    const auto initialized = p->initialize(r, i);
    const auto operation = p->operation();
    const auto disposed = p->dispose();
    const auto restored = SetThreadDpiAwarenessContext(previous);
    if (!accepted_color(i.color_space, i.bits_per_color, i.advanced_color_flags)) {
        std::fprintf(stderr,
                     "native enrollment HRESULT=0x%08lx operation=%u color=%u bits=%u flags=%u\n",
                     static_cast<unsigned long>(initialized), operation, i.color_space,
                     i.bits_per_color, i.advanced_color_flags);
        check(initialized == E_NOTIMPL && operation == SS_DXGI_COLOR,
              "Actual advanced-color output not rejected");
    } else {
        check(SUCCEEDED(initialized), "Real exact-output/D3D/fence/staging enrollment failed");
        check(i.staging_width == 2 && i.staging_height == 2 && i.texture_format == 87 &&
                  i.qpc_frequency > 0,
              "Native device/staging metadata inaccurate");
    }
    check(SUCCEEDED(disposed) && restored != nullptr,
          "Real partial/native resource cleanup failed");
    std::printf("native enrollment result=0x%08lx color=%u bits=%u flags=%u staging_format=%u\n",
                static_cast<unsigned long>(initialized), i.color_space, i.bits_per_color,
                i.advanced_color_flags, i.texture_format);
}
void explicit_sdr_and_os_bgra_hdr_cohorts_reject_unknown_modes() {
    check(accepted_color(0, 8, 0) && accepted_color(0, 8, 1), "Known SDR rejected");
    check(accepted_color(12, 10, 2) && accepted_color(12, 10, 3),
          "Observed HDR10 OS-BGRA cohort rejected");
    for (const auto invalid : {uint32_t{0}, uint32_t{1}, uint32_t{7}, uint32_t{11}, uint32_t{19}})
        check(!accepted_color(12, 10, invalid),
              "Unknown/inconsistent advanced-color flags accepted");
    check(!accepted_color(0, 8, 3) && !accepted_color(12, 8, 3) && !accepted_color(1, 10, 3),
          "Unsupported gamut/bitdepth accepted");
}
void tight_rows_preserve_bgra_and_ignore_padding() {
    std::array<uint8_t, 40> data{};
    for (uint8_t i = 0; i < 8; ++i) {
        data[i] = i;
        data[20 + i] = 8 + i;
    }
    ss_dxgi_sample s{};
    check(copy_rows(data.data(), 20, 2, 2, s) == S_OK && s.byte_count == 16,
          "Padded rows rejected");
    for (uint8_t i = 0; i < 16; ++i)
        check(s.bgra[i] == i, "Channel/padding/row ordering corrupted");
    check(FAILED(copy_rows(data.data(), 7, 2, 2, s)), "Invalid RowPitch accepted");
    check(FAILED(copy_rows(nullptr, 20, 2, 2, s)), "Null Map accepted");
}
void real_device_staging_fence_without_desktop_capture() {
    using Microsoft::WRL::ComPtr;
    ComPtr<IDXGIFactory1> factory;
    check(SUCCEEDED(CreateDXGIFactory1(IID_PPV_ARGS(&factory))), "Native factory creation failed");
    ComPtr<IDXGIAdapter1> selected;
    const POINT origin{};
    const auto monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
    for (UINT a = 0; a < 64 && !selected; ++a) {
        ComPtr<IDXGIAdapter1> adapter;
        const auto hr = factory->EnumAdapters1(a, &adapter);
        if (hr == DXGI_ERROR_NOT_FOUND)
            break;
        check(SUCCEEDED(hr), "Native adapter enumeration failed");
        for (UINT o = 0; o < 64; ++o) {
            ComPtr<IDXGIOutput> output;
            const auto result = adapter->EnumOutputs(o, &output);
            if (result == DXGI_ERROR_NOT_FOUND)
                break;
            check(SUCCEEDED(result), "Native output enumeration failed");
            DXGI_OUTPUT_DESC desc{};
            check(SUCCEEDED(output->GetDesc(&desc)), "Native output description failed");
            if (desc.Monitor == monitor) {
                selected = adapter;
                break;
            }
        }
    }
    check(selected != nullptr, "Exact native output adapter unavailable");
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    check(SUCCEEDED(D3D11CreateDevice(selected.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
                                      D3D11_CREATE_DEVICE_BGRA_SUPPORT |
                                          D3D11_CREATE_DEVICE_SINGLETHREADED,
                                      nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &context)),
          "Exact-adapter D3D device creation failed");
    ComPtr<ID3D11Device5> modern;
    ComPtr<ID3D11Fence> fence;
    ComPtr<ID3D11Texture2D> staging;
    check(SUCCEEDED(device.As(&modern)), "Native fence API unsupported");
    check(SUCCEEDED(modern->CreateFence(0, D3D11_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence))),
          "Native fence creation failed");
    D3D11_TEXTURE2D_DESC desc{};
    desc.Width = 2;
    desc.Height = 2;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.ArraySize = 1;
    desc.MipLevels = 1;
    desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_STAGING;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    check(SUCCEEDED(device->CreateTexture2D(&desc, nullptr, &staging)),
          "Native fixed BGRA staging creation failed");
    check(fence->GetCompletedValue() == 0, "Unexpected initial fence state");
}
} // namespace
void run_native_tests() {
    native_enrollment_and_release_without_capture();
    tight_rows_preserve_bgra_and_ignore_padding();
    real_device_staging_fence_without_desktop_capture();
    explicit_sdr_and_os_bgra_hdr_cohorts_reject_unknown_modes();
}
