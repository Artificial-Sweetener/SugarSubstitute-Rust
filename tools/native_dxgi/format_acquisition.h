// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include <cstdint>
#include <d3d11.h>
#include <dxgi1_2.h>
struct FormatAcquisition {
    HRESULT acquire = S_FALSE, query = S_FALSE, release = S_FALSE, clock = S_OK;
    HRESULT primary = S_OK, secondary = S_OK;
    uint32_t primary_operation = 0, secondary_operation = 0;
    bool attempted = false, acquired = false;
    UINT timeout_ms = 0;
    int64_t frequency = 0, begin_qpc = 0, end_qpc = 0;
    DXGI_OUTDUPL_FRAME_INFO frame{};
    D3D11_TEXTURE2D_DESC texture{};
};
// Only an observed timeout or a completely released/queryable frame permits the
// diagnostic's next acquisition. Unknown release/clock state ends the process.
inline bool format_acquisition_settled(const FormatAcquisition &result) noexcept {
    if (!result.attempted || result.clock != S_OK)
        return false;
    if (result.acquire == DXGI_ERROR_WAIT_TIMEOUT)
        return !result.acquired && result.query == S_FALSE && result.release == S_FALSE;
    return result.acquire == S_OK && result.acquired && result.query == S_OK &&
           result.release == S_OK;
}
// Exactly one finite acquisition and one release for each actual acquired frame.
// This diagnostic inspects a description only, never copies/maps/emits pixels.
void inspect_next_format(IDXGIOutputDuplication &duplication, UINT timeout,
                         FormatAcquisition &result) noexcept;
void print_format_acquisition(const char *key, HRESULT enrollment,
                              const FormatAcquisition &result) noexcept;
