// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include "native_dxgi.h"
#include <Windows.h>
#include <cstring>

namespace ss::dxgi {
// The native Map boundary owns a valid height-row allocation. Copy tight BGRA rows
// only; padding is never observable data. Caller always Unmaps after this returns.
inline HRESULT copy_rows(const void *mapped, uint32_t pitch, uint32_t width, uint32_t height,
                         ss_dxgi_sample &sample) noexcept {
    if (!mapped || width < 2 || width > 16 || height < 2 || height > 16 || pitch < width * 4)
        return E_INVALIDARG;
    const uint32_t row_bytes = width * 4;
    const auto *pixels = static_cast<const uint8_t *>(mapped);
    for (uint32_t row = 0; row < height; ++row)
        memcpy(sample.bgra + row * row_bytes, pixels + static_cast<size_t>(row) * pitch, row_bytes);
    sample.byte_count = row_bytes * height;
    return S_OK;
}
} // namespace ss::dxgi
