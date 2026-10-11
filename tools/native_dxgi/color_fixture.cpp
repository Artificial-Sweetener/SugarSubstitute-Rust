// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Independent opaque SDK palette witness. Only root runs this visible fixture;
// properties/diagnostics establish identity, never substitute for observed pixels.
#include <Windows.h>
#include <array>
#include <cstdint>
#include <cstdio>

namespace {
constexpr int button_id = 1001;
constexpr int roi_x = 40, roi_y = 40;
constexpr std::array<COLORREF, 4> initial{RGB(31, 37, 43), RGB(43, 211, 59), RGB(71, 83, 227),
                                          RGB(191, 137, 101)};
constexpr std::array<COLORREF, 4> changed{RGB(229, 233, 237), RGB(173, 47, 113), RGB(89, 229, 151),
                                          RGB(251, 193, 67)};
struct Fixture {
    HWND button = nullptr;
    uint32_t revision = 1, painted = 0;
    bool down = false;
    bool failed = false;
};
void startup_diagnostic(const char *operation, DWORD error) noexcept {
    std::fprintf(stderr,
                 "{\"event\":\"native_startup_failure\",\"api\":\"%s\",\"win32_error\":%lu}\n",
                 operation, error);
    std::fflush(stderr);
}
int startup_failure(const char *operation, DWORD error, const wchar_t *registered_class,
                    HINSTANCE instance) noexcept {
    // Preserve the immediate primary error before any fallible cleanup call.
    startup_diagnostic(operation, error);
    if (registered_class && !UnregisterClassW(registered_class, instance))
        startup_diagnostic("UnregisterClassW", GetLastError());
    return 1;
}
void fail(Fixture &f, HWND window, const char *operation) noexcept {
    if (!f.failed)
        std::fprintf(stderr, "color fixture %s failed: %lu\n", operation, GetLastError());
    f.failed = true;
    PostMessageW(window, WM_CLOSE, 0, 0);
}
void change(Fixture &f, HWND window) noexcept {
    if (f.revision != 1)
        return;
    f.revision = 2;
    if (!InvalidateRect(window, nullptr, FALSE))
        fail(f, window, "invalidate");
}
bool fill(HDC dc, const RECT &rect, COLORREF color) noexcept {
    const auto brush = CreateSolidBrush(color);
    if (!brush)
        return false;
    const bool painted = FillRect(dc, &rect, brush) != 0;
    const bool destroyed = DeleteObject(brush) != FALSE;
    return painted && destroyed;
}
void paint(Fixture &f, HWND window) noexcept {
    PAINTSTRUCT ps{};
    const auto dc = BeginPaint(window, &ps);
    if (!dc) {
        fail(f, window, "begin_paint");
        return;
    }
    RECT client{};
    bool okay = GetClientRect(window, &client) != FALSE && fill(dc, client, RGB(35, 39, 43));
    const auto &palette = f.revision == 1 ? initial : changed;
    for (size_t i = 0; i < palette.size(); ++i) {
        const auto x = roi_x + static_cast<LONG>(i % 2), y = roi_y + static_cast<LONG>(i / 2);
        const RECT pixel{x, y, x + 1, y + 1};
        okay = fill(dc, pixel, palette[i]) && okay;
    }
    okay = EndPaint(window, &ps) != FALSE && okay;
    if (!okay) {
        fail(f, window, "paint");
        return;
    }
    if (f.painted == f.revision)
        return;
    LARGE_INTEGER ticks{}, frequency{};
    POINT origin{roi_x, roi_y};
    if (!QueryPerformanceCounter(&ticks) || !QueryPerformanceFrequency(&frequency) ||
        !ClientToScreen(window, &origin) || ticks.QuadPart <= 0 || frequency.QuadPart <= 0) {
        fail(f, window, "paint_identity");
        return;
    }
    if (!SetPropW(window, L"SS.DxgiPaintRevision",
                  reinterpret_cast<HANDLE>(static_cast<uintptr_t>(f.revision))) ||
        !SetPropW(window, L"SS.DxgiPaintQpc", reinterpret_cast<HANDLE>(ticks.QuadPart))) {
        fail(f, window, "paint_properties");
        return;
    }
    f.painted = f.revision;
    std::printf("{\"event\":\"painted\",\"pid\":%lu,\"hwnd\":%llu,\"thread\":%lu,\"revision\":%u,"
                "\"paint_qpc\":%lld,\"qpc_frequency\":%lld,\"roi\":{\"x\":%ld,\"y\":%ld,\"width\":"
                "2,\"height\":2},\"button_id\":1001,\"bgra\":[",
                GetCurrentProcessId(), reinterpret_cast<unsigned long long>(window),
                GetCurrentThreadId(), f.revision, ticks.QuadPart, frequency.QuadPart, origin.x,
                origin.y);
    for (size_t i = 0; i < palette.size(); ++i)
        std::printf("%s[%u,%u,%u,255]", i ? "," : "", GetBValue(palette[i]), GetGValue(palette[i]),
                    GetRValue(palette[i]));
    std::puts("]}");
    std::fflush(stdout);
}
LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam) noexcept {
    auto *f = reinterpret_cast<Fixture *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        const auto *creation = reinterpret_cast<CREATESTRUCTW *>(lparam);
        f = static_cast<Fixture *>(creation->lpCreateParams);
        SetLastError(ERROR_SUCCESS);
        if (!SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(f))) {
            const auto error = GetLastError();
            if (error) {
                startup_diagnostic("SetWindowLongPtrW(GWLP_USERDATA)", error);
                SetLastError(error);
                return FALSE;
            }
        }
    }
    if (!f)
        return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_PAINT:
        paint(*f, window);
        return 0;
    case WM_LBUTTONDOWN:
        f->down = true;
        return 0;
    case WM_LBUTTONUP:
        if (f->down) {
            f->down = false;
            change(*f, window);
        }
        return 0;
    case WM_COMMAND:
        if (LOWORD(wparam) == button_id && HIWORD(wparam) == BN_CLICKED &&
            reinterpret_cast<HWND>(lparam) == f->button) {
            change(*f, window);
            return 0;
        }
        break;
    case WM_CLOSE:
        if (!DestroyWindow(window)) {
            fail(*f, window, "destroy_window");
            PostQuitMessage(1);
        }
        return 0;
    case WM_DESTROY:
        PostQuitMessage(f->failed ? 1 : 0);
        return 0;
    case WM_NCDESTROY:
        RemovePropW(window, L"SS.DxgiPaintRevision");
        RemovePropW(window, L"SS.DxgiPaintQpc");
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        break;
    default:
        break;
    }
    return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace
int wmain() {
    if (!SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2))
        return startup_failure("SetProcessDpiAwarenessContext", GetLastError(), nullptr, nullptr);
    const auto instance = GetModuleHandleW(nullptr);
    if (!instance)
        return startup_failure("GetModuleHandleW", GetLastError(), nullptr, nullptr);
    WNDCLASSW type{};
    type.lpfnWndProc = procedure;
    type.hInstance = instance;
    type.lpszClassName = L"SS.DxgiColorFixture.V1";
    type.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    if (!type.hCursor)
        return startup_failure("LoadCursorW", GetLastError(), nullptr, instance);
    if (!RegisterClassW(&type))
        return startup_failure("RegisterClassW", GetLastError(), nullptr, instance);
    constexpr DWORD style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    RECT bounds{0, 0, 320, 160};
    if (!AdjustWindowRectEx(&bounds, style, FALSE, 0))
        return startup_failure("AdjustWindowRectEx", GetLastError(), type.lpszClassName, instance);
    Fixture fixture;
    const auto window =
        CreateWindowExW(0, type.lpszClassName, L"SugarSubstitute DXGI color fixture", style,
                        CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left,
                        bounds.bottom - bounds.top, nullptr, nullptr, instance, &fixture);
    if (!window)
        return startup_failure("CreateWindowExW(parent)", GetLastError(), type.lpszClassName,
                               instance);
    fixture.button = CreateWindowExW(
        0, L"BUTTON", L"Change palette", WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON, 150,
        55, 140, 35, window, reinterpret_cast<HMENU>(static_cast<INT_PTR>(button_id)), instance,
        nullptr);
    if (!fixture.button) {
        const auto error = GetLastError();
        startup_diagnostic("CreateWindowExW(BUTTON)", error);
        if (!DestroyWindow(window))
            startup_diagnostic("DestroyWindow", GetLastError());
        if (!UnregisterClassW(type.lpszClassName, instance))
            startup_diagnostic("UnregisterClassW", GetLastError());
        return 1;
    }
    ShowWindow(window, SW_SHOW);
    SetForegroundWindow(window);
    MSG message{};
    BOOL result = 0;
    while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    if (result < 0) {
        const auto error = GetLastError();
        startup_diagnostic("GetMessageW", error);
        if (IsWindow(window) && !DestroyWindow(window))
            startup_diagnostic("DestroyWindow", GetLastError());
        if (!UnregisterClassW(type.lpszClassName, instance))
            startup_diagnostic("UnregisterClassW", GetLastError());
        return 1;
    }
    if (!UnregisterClassW(type.lpszClassName, instance)) {
        startup_diagnostic("UnregisterClassW", GetLastError());
        return 1;
    }
    return static_cast<int>(message.wParam);
}
