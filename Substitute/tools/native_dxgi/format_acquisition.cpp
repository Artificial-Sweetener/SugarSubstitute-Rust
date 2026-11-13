// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "format_acquisition.h"
#include "native_dxgi.h"
#include "native_error.h"
#include <cstdio>
#include <wrl/client.h>
namespace {
void observe(FormatAcquisition &result, HRESULT cause, uint32_t operation) noexcept {
    if (!FAILED(cause))
        return;
    if (SUCCEEDED(result.primary)) {
        result.primary = cause;
        result.primary_operation = operation;
    } else if (SUCCEEDED(result.secondary)) {
        result.secondary = cause;
        result.secondary_operation = operation;
    }
}
HRESULT counter(int64_t &ticks, int64_t previous) noexcept {
    LARGE_INTEGER value{};
    if (!QueryPerformanceCounter(&value))
        return ss::dxgi::win32_failure(GetLastError());
    ticks = value.QuadPart;
    return ticks > 0 && ticks >= previous ? S_OK : E_UNEXPECTED;
}
} // namespace
void inspect_next_format(IDXGIOutputDuplication &duplication, UINT timeout,
                         FormatAcquisition &result) noexcept {
    result.timeout_ms = timeout;
    if (timeout == 0 || timeout > 500) {
        observe(result, E_INVALIDARG, SS_DXGI_ARGUMENT);
        return;
    }
    LARGE_INTEGER frequency{};
    if (!QueryPerformanceFrequency(&frequency))
        result.clock = ss::dxgi::win32_failure(GetLastError());
    else if (frequency.QuadPart <= 0)
        result.clock = E_UNEXPECTED;
    result.frequency = frequency.QuadPart;
    if (SUCCEEDED(result.clock))
        result.clock = counter(result.begin_qpc, 0);
    observe(result, result.clock, SS_DXGI_QPC);
    if (FAILED(result.clock))
        return;
    Microsoft::WRL::ComPtr<IDXGIResource> resource;
    result.attempted = true;
    result.acquire = duplication.AcquireNextFrame(timeout, &result.frame, &resource);
    result.acquired = result.acquire == S_OK;
    observe(result, result.acquire, SS_DXGI_ACQUIRE);
    result.clock = counter(result.end_qpc, result.begin_qpc);
    observe(result, result.clock, SS_DXGI_QPC);
    if (!result.acquired)
        return;
    Microsoft::WRL::ComPtr<ID3D11Texture2D> source;
    result.query = resource.As(&source);
    observe(result, result.query, SS_DXGI_ACQUIRE);
    if (result.query == S_OK)
        source->GetDesc(&result.texture);
    // The lease survives every metadata failure until this exactly-once attempt.
    result.release = duplication.ReleaseFrame();
    observe(result, result.release, SS_DXGI_RELEASE);
}
void print_format_acquisition(const char *key, HRESULT enrollment,
                              const FormatAcquisition &r) noexcept {
    std::printf("\"%s\":{\"enroll_hresult\":%ld,\"attempted\":%s,\"acquire_hresult\":%ld,\"query_"
                "hresult\":%ld,"
                "\"release_hresult\":%ld,\"clock_hresult\":%ld,\"primary_hresult\":%ld,\"primary_"
                "operation\":%u,"
                "\"secondary_hresult\":%ld,\"secondary_operation\":%u,\"frame_acquired\":%s,"
                "\"release_attempts\":%u,"
                "\"timeout_ms\":%u,\"qpc_frequency\":%lld,\"acquire_begin_qpc\":%lld,\"acquire_end_"
                "qpc\":%lld,"
                "\"last_present_qpc\":%lld,\"last_mouse_qpc\":%lld,\"accumulated_frames\":%u,"
                "\"protected_content_masked\":%s,\"texture_format\":%u,\"width\":%u,\"height\":%u,"
                "\"mip_levels\":%u,\"array_size\":%u,\"sample_count\":%u}",
                key, enrollment, r.attempted ? "true" : "false", r.acquire, r.query, r.release,
                r.clock, r.primary, r.primary_operation, r.secondary, r.secondary_operation,
                r.acquired ? "true" : "false", r.acquired ? 1u : 0u, r.timeout_ms, r.frequency,
                r.begin_qpc, r.end_qpc, r.frame.LastPresentTime.QuadPart,
                r.frame.LastMouseUpdateTime.QuadPart, r.frame.AccumulatedFrames,
                r.frame.ProtectedContentMaskedOut ? "true" : "false",
                static_cast<UINT>(r.texture.Format), r.texture.Width, r.texture.Height,
                r.texture.MipLevels, r.texture.ArraySize, r.texture.SampleDesc.Count);
}
