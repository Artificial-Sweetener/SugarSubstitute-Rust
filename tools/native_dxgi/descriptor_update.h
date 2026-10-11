// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include <Windows.h>
#include <cstdint>

// A diagnostic-owned, nonactivating desktop update; never a pixel observer.
// Keep this creating-thread state alive through acquire and explicit close.
struct DesktopUpdate {
    HWND window = nullptr;
    ATOM registered_class = 0;
    HRESULT create = S_FALSE, paint = S_FALSE, destroy = S_FALSE, unregister = S_FALSE;
    HRESULT visibility = S_FALSE;
    HRESULT gdi_flush = S_FALSE, dwm_flush = S_FALSE, flush_clock = S_OK;
    int64_t gdi_begin_qpc = 0, gdi_end_qpc = 0, dwm_begin_qpc = 0, dwm_end_qpc = 0;
    HWND foreground_before = nullptr, foreground_enrolled = nullptr, foreground_after = nullptr;
    RECT observed_window{}, observed_client{};
    bool visible = false, topmost = false, owned_points = false;
    uint32_t paint_count = 0;
    int64_t paint_qpc = 0;
};
bool begin_desktop_update(DesktopUpdate &update) noexcept;
void close_desktop_update(DesktopUpdate &update) noexcept;
