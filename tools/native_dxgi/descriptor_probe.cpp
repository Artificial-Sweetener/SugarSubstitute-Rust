// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Descriptor-only default. Root may explicitly request one bounded format-only
// acquisition; neither path copies/maps/emits pixels or establishes color fidelity.
#include "descriptor_update.h"
#include "desktop_identity.h"
#include "format_acquisition.h"
#include <Windows.h>
#include <cstdio>
#include <cstring>
#include <d3d11.h>
#include <dxgi1_6.h>
#include <wrl/client.h>

using Microsoft::WRL::ComPtr;
// Diagnostic-only timeout. Observer ABI acquisition remains capped at 5 ms.
constexpr UINT format_probe_timeout_ms = 500;
int main(int argc, char **argv) {
    if (argc == 2 && std::strcmp(argv[1], "--desktop-identity-only") == 0)
        return print_desktop_identity();
    const bool hdr_inventory =
        argc == 2 && std::strcmp(argv[1], "--acquire-format-hdr-inventory-owned-update-once") == 0;
    const bool after_first =
        argc == 2 && std::strcmp(argv[1], "--acquire-format-update-after-first-once") == 0;
    const bool owned_update =
        hdr_inventory || after_first ||
        (argc == 2 && std::strcmp(argv[1], "--acquire-format-owned-update-once") == 0);
    const bool acquire_format =
        owned_update || (argc == 2 && std::strcmp(argv[1], "--acquire-format-once") == 0);
    if (argc != 1 && !acquire_format)
        return 2;
    const auto prior = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    if (!prior)
        return 1;
    const POINT origin{};
    const auto monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
    ComPtr<IDXGIFactory1> factory;
    auto result = CreateDXGIFactory1(IID_PPV_ARGS(&factory));
    if (FAILED(result))
        return 1;
    ComPtr<IDXGIAdapter1> selected;
    ComPtr<IDXGIOutput6> output;
    UINT output_index = 0;
    for (UINT a = 0; a < 64 && !selected; ++a) {
        ComPtr<IDXGIAdapter1> adapter;
        result = factory->EnumAdapters1(a, &adapter);
        if (result == DXGI_ERROR_NOT_FOUND)
            break;
        if (FAILED(result))
            return 1;
        for (UINT o = 0; o < 64; ++o) {
            ComPtr<IDXGIOutput> candidate;
            result = adapter->EnumOutputs(o, &candidate);
            if (result == DXGI_ERROR_NOT_FOUND)
                break;
            if (FAILED(result))
                return 1;
            DXGI_OUTPUT_DESC desc{};
            if (FAILED(candidate->GetDesc(&desc)))
                return 1;
            if (desc.Monitor != monitor)
                continue;
            selected = adapter;
            output_index = o;
            if (FAILED(candidate.As(&output)))
                return 1;
            break;
        }
    }
    if (!selected || !output)
        return 1;
    DXGI_OUTPUT_DESC1 desc{};
    DXGI_ADAPTER_DESC1 adapter_desc{};
    if (FAILED(output->GetDesc1(&desc)) || FAILED(selected->GetDesc1(&adapter_desc)))
        return 1;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> immediate;
    result =
        D3D11CreateDevice(selected.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
                          D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                          nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &immediate);
    if (FAILED(result))
        return 1;
    ComPtr<IDXGIOutput1> output1;
    if (FAILED(output.As(&output1)))
        return 1;
    ComPtr<IDXGIOutputDuplication> duplication;
    DXGI_OUTDUPL_DESC original{}, requested{}, hdr_requested{};
    HRESULT original_hr = S_FALSE, hdr_hr = S_FALSE;
    if (hdr_inventory) {
        ComPtr<IDXGIOutput5> modern;
        if (FAILED(output.As(&modern)))
            return 1;
        // Exact modern Microsoft sample format inventory, no conversion by us.
        constexpr DXGI_FORMAT formats[]{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM,
                                        DXGI_FORMAT_R16G16B16A16_FLOAT};
        hdr_hr = modern->DuplicateOutput1(device.Get(), 0, 3, formats, &duplication);
        if (hdr_hr == S_OK)
            duplication->GetDesc(&hdr_requested);
    } else {
        original_hr = output1->DuplicateOutput(device.Get(), &duplication);
        if (original_hr == S_OK)
            duplication->GetDesc(&original);
    }
    HRESULT requested_hr = S_FALSE;
    if (!acquire_format) {
        // Descriptor comparison is a separate mode; acquisition retains ONLY the
        // first original enrollment, matching the production observer lifecycle.
        duplication.Reset();
        ComPtr<IDXGIOutput5> output5;
        if (FAILED(output.As(&output5)))
            return 1;
        const DXGI_FORMAT bgra = DXGI_FORMAT_B8G8R8A8_UNORM;
        requested_hr = output5->DuplicateOutput1(device.Get(), 0, 1, &bgra, &duplication);
        if (requested_hr == S_OK)
            duplication->GetDesc(&requested);
        duplication.Reset();
    }
    const HRESULT enrolled_hr = hdr_inventory ? hdr_hr : original_hr;
    FormatAcquisition first{}, observed{};
    DesktopUpdate update{};
    if (acquire_format) {
        if (enrolled_hr == S_OK) {
            if (after_first)
                inspect_next_format(*duplication.Get(), format_probe_timeout_ms, first);
            if ((!after_first || format_acquisition_settled(first)) &&
                (!owned_update || begin_desktop_update(update)))
                inspect_next_format(*duplication.Get(), format_probe_timeout_ms, observed);
        }
        duplication.Reset();
        close_desktop_update(update);
    }
    if (!SetThreadDpiAwarenessContext(prior))
        return 1;
    std::printf(
        "{\"schema_version\":1,\"acquire_format_requested\":%s,\"no_pixel_copy_or_map\":true,"
        "\"duplication_enrollments\":%u,\"duplicate_output1_attempted\":%s,"
        "\"original_duplicate_output_attempted\":%s,\"acquisition_enrollment_api\":\"%s\","
        "\"acquisition_requested_formats\":%s,\"acquisition_enrollment_flags\":0,"
        "\"thread\":%lu,\"monitor\":%llu,"
        "\"output_index\":%u,\"adapter_luid_low\":%lu,\"adapter_luid_high\":%ld,\"vendor_id\":%u,"
        "\"device_id\":%u,\"output_rect\":[%ld,%ld,%ld,%ld],\"rotation\":%u,\"color_space\":%u,"
        "\"bits_per_color\":%u,\"duplicate_output\":{\"hresult\":%ld,\"format\":%u,\"width\":%u,"
        "\"height\":%u,\"rotation\":%u},\"duplicate_output1_bgra_only\":{\"hresult\":%ld,"
        "\"format\":%u,\"width\":%u,\"height\":%u,\"rotation\":%u},"
        "\"duplicate_output1_hdr_inventory\":{\"attempted\":%s,\"hresult\":%ld,\"format\":%u,"
        "\"width\":%u,\"height\":%u,\"rotation\":%u},",
        acquire_format ? "true" : "false", acquire_format ? 1u : 2u,
        hdr_inventory || !acquire_format ? "true" : "false", hdr_inventory ? "false" : "true",
        acquire_format ? (hdr_inventory ? "DuplicateOutput1" : "DuplicateOutput") : "none",
        hdr_inventory ? "[28,87,10]" : "[]", GetCurrentThreadId(),
        reinterpret_cast<unsigned long long>(monitor), output_index,
        adapter_desc.AdapterLuid.LowPart, adapter_desc.AdapterLuid.HighPart, adapter_desc.VendorId,
        adapter_desc.DeviceId, desc.DesktopCoordinates.left, desc.DesktopCoordinates.top,
        desc.DesktopCoordinates.right, desc.DesktopCoordinates.bottom,
        static_cast<UINT>(desc.Rotation), static_cast<UINT>(desc.ColorSpace), desc.BitsPerColor,
        original_hr, static_cast<UINT>(original.ModeDesc.Format), original.ModeDesc.Width,
        original.ModeDesc.Height, static_cast<UINT>(original.Rotation), requested_hr,
        static_cast<UINT>(requested.ModeDesc.Format), requested.ModeDesc.Width,
        requested.ModeDesc.Height, static_cast<UINT>(requested.Rotation),
        hdr_inventory ? "true" : "false", hdr_hr, static_cast<UINT>(hdr_requested.ModeDesc.Format),
        hdr_requested.ModeDesc.Width, hdr_requested.ModeDesc.Height,
        static_cast<UINT>(hdr_requested.Rotation));
    std::printf("\"acquisition_attempts_requested\":%u,", !acquire_format ? 0u
                                                          : after_first   ? 2u
                                                                          : 1u);
    print_format_acquisition("first_before_owned_update_acquire", enrolled_hr, first);
    std::putchar(',');
    print_format_acquisition("format_only_acquire", enrolled_hr, observed);
    std::putchar(',');
    std::printf(
        "\"owned_desktop_update\":{\"requested\":%s,\"create_hresult\":%ld,\"paint_hresult\":%ld,"
        "\"destroy_hresult\":%ld,\"unregister_hresult\":%ld,\"paint_count\":%u,\"paint_qpc\":%lld,"
        "\"window_retained\":%s,\"class_retained\":%s,\"visibility_hresult\":%ld,"
        "\"visible\":%s,\"topmost\":%s,\"owned_point_witnesses\":%s,"
        "\"window_rect\":[%ld,%ld,%ld,%ld],\"client_rect\":[%ld,%ld,%ld,%ld],"
        "\"foreground_before\":%llu,\"foreground_enrolled\":%llu,\"foreground_after\":%llu,"
        "\"foreground_preserved\":%s,\"gdi_flush_hresult\":%ld,\"dwm_flush_hresult\":%ld,"
        "\"flush_clock_hresult\":%ld,\"gdi_begin_qpc\":%lld,\"gdi_end_qpc\":%lld,"
        "\"dwm_begin_qpc\":%lld,\"dwm_end_qpc\":%lld}}\n",
        owned_update ? "true" : "false", update.create, update.paint, update.destroy,
        update.unregister, update.paint_count, update.paint_qpc, update.window ? "true" : "false",
        update.registered_class ? "true" : "false", update.visibility,
        update.visible ? "true" : "false", update.topmost ? "true" : "false",
        update.owned_points ? "true" : "false", update.observed_window.left,
        update.observed_window.top, update.observed_window.right, update.observed_window.bottom,
        update.observed_client.left, update.observed_client.top, update.observed_client.right,
        update.observed_client.bottom,
        reinterpret_cast<unsigned long long>(update.foreground_before),
        reinterpret_cast<unsigned long long>(update.foreground_enrolled),
        reinterpret_cast<unsigned long long>(update.foreground_after),
        update.foreground_before == update.foreground_enrolled &&
                update.foreground_before == update.foreground_after
            ? "true"
            : "false",
        update.gdi_flush, update.dwm_flush, update.flush_clock, update.gdi_begin_qpc,
        update.gdi_end_qpc, update.dwm_begin_qpc, update.dwm_end_qpc);
    if (acquire_format && (enrolled_hr != S_OK || !format_acquisition_settled(observed) ||
                           (after_first && !format_acquisition_settled(first))))
        return 1;
    return owned_update &&
                   (update.create != S_OK || update.paint != S_OK || update.destroy != S_OK ||
                    update.unregister != S_OK || update.visibility != S_OK ||
                    update.foreground_before != update.foreground_after ||
                    update.gdi_flush != S_OK || update.dwm_flush != S_OK ||
                    update.flush_clock != S_OK)
               ? 1
               : 0;
}
