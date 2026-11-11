// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "descriptor_update.h"
#include "native_error.h"
#include <cstdint>
#include <dwmapi.h>

namespace {
constexpr wchar_t class_name[] = L"SugarSubstitute.DxgiFormatUpdate.V1";

bool flush_stamp(DesktopUpdate &state, int64_t &ticks, int64_t previous) noexcept {
    LARGE_INTEGER value{};
    if (!QueryPerformanceCounter(&value)) {
        state.flush_clock = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    ticks = value.QuadPart;
    if (ticks < previous || ticks <= 0) {
        state.flush_clock = E_UNEXPECTED;
        return false;
    }
    return true;
}
LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam) noexcept {
    auto *state = reinterpret_cast<DesktopUpdate *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        state =
            static_cast<DesktopUpdate *>(reinterpret_cast<CREATESTRUCTW *>(lparam)->lpCreateParams);
        SetLastError(0);
        if (!SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state)) &&
            GetLastError())
            return FALSE;
    }
    if (message == WM_PAINT && state) {
        PAINTSTRUCT paint{};
        const auto dc = BeginPaint(window, &paint);
        if (!dc) {
            state->paint = ss::dxgi::win32_failure(GetLastError());
            return 0;
        }
        RECT bounds{};
        HRESULT result = S_OK;
        if (!GetClientRect(window, &bounds) ||
            !FillRect(dc, &bounds, GetSysColorBrush(COLOR_HIGHLIGHT)))
            result = ss::dxgi::win32_failure(GetLastError());
        if (!EndPaint(window, &paint) && SUCCEEDED(result))
            result = ss::dxgi::win32_failure(GetLastError());
        LARGE_INTEGER tick{};
        if (!QueryPerformanceCounter(&tick) && SUCCEEDED(result))
            result = ss::dxgi::win32_failure(GetLastError());
        else if (tick.QuadPart <= 0 && SUCCEEDED(result))
            result = E_UNEXPECTED;
        if (SUCCEEDED(result)) {
            ++state->paint_count;
            state->paint_qpc = tick.QuadPart;
        }
        if (state->paint == S_FALSE || SUCCEEDED(state->paint))
            state->paint = result;
        return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace
bool begin_desktop_update(DesktopUpdate &state) noexcept {
    state.foreground_before = GetForegroundWindow();
    WNDCLASSW klass{};
    klass.lpfnWndProc = procedure;
    klass.hInstance = GetModuleHandleW(nullptr);
    klass.lpszClassName = class_name;
    state.registered_class = RegisterClassW(&klass);
    if (!state.registered_class) {
        state.create = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    state.window = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST, class_name,
                                   L"SugarSubstitute DXGI format diagnostic", WS_POPUP, 80, 80, 48,
                                   48, nullptr, nullptr, klass.hInstance, &state);
    if (!state.window) {
        state.create = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    state.create = S_OK;
    ShowWindow(state.window, SW_SHOWNOACTIVATE);
    if (!SetWindowPos(state.window, HWND_TOPMOST, 0, 0, 0, 0,
                      SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW)) {
        state.visibility = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    if (!IsWindowVisible(state.window) || !UpdateWindow(state.window)) {
        if (SUCCEEDED(state.paint))
            state.paint = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    state.foreground_enrolled = GetForegroundWindow();
    state.visible = IsWindowVisible(state.window) != FALSE;
    state.topmost = (GetWindowLongPtrW(state.window, GWL_EXSTYLE) & WS_EX_TOPMOST) != 0;
    RECT client{};
    POINT client_origin{};
    if (!GetWindowRect(state.window, &state.observed_window) ||
        !GetClientRect(state.window, &client) || !ClientToScreen(state.window, &client_origin)) {
        state.visibility = ss::dxgi::win32_failure(GetLastError());
        return false;
    }
    state.observed_client = {client_origin.x, client_origin.y, client_origin.x + client.right,
                             client_origin.y + client.bottom};
    state.owned_points = true;
    constexpr POINT witnesses[]{{82, 82}, {125, 82}, {82, 125}, {125, 125}};
    for (const auto point : witnesses)
        state.owned_points = state.owned_points && WindowFromPoint(point) == state.window;
    const RECT expected{80, 80, 128, 128};
    const bool geometry = EqualRect(&state.observed_window, &expected) &&
                          EqualRect(&state.observed_client, &expected);
    state.visibility = state.visible && state.topmost && state.owned_points && geometry &&
                               state.foreground_before == state.foreground_enrolled
                           ? S_OK
                           : E_FAIL;
    if (state.paint != S_OK || state.paint_count == 0 || state.visibility != S_OK)
        return false;
    if (!flush_stamp(state, state.gdi_begin_qpc, state.paint_qpc))
        return false;
    state.gdi_flush = GdiFlush() ? S_OK : ss::dxgi::win32_failure(GetLastError());
    const bool gdi_clock = flush_stamp(state, state.gdi_end_qpc, state.gdi_begin_qpc);
    if (state.gdi_flush != S_OK || !gdi_clock)
        return false;
    if (!flush_stamp(state, state.dwm_begin_qpc, state.gdi_end_qpc))
        return false;
    state.dwm_flush = DwmFlush();
    const bool dwm_clock = flush_stamp(state, state.dwm_end_qpc, state.dwm_begin_qpc);
    return state.dwm_flush == S_OK && dwm_clock;
}
void close_desktop_update(DesktopUpdate &state) noexcept {
    if (state.window) {
        if (DestroyWindow(state.window)) {
            state.window = nullptr;
            state.destroy = S_OK;
        } else {
            state.destroy = ss::dxgi::win32_failure(GetLastError());
        }
    }
    if (state.registered_class && !state.window) {
        if (UnregisterClassW(class_name, GetModuleHandleW(nullptr))) {
            state.registered_class = 0;
            state.unregister = S_OK;
        } else {
            state.unregister = ss::dxgi::win32_failure(GetLastError());
        }
    }
    state.foreground_after = GetForegroundWindow();
}
