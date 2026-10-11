// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "desktop_identity.h"
#include "native_error.h"
#include <Windows.h>
#include <cstdio>
#include <cwchar>

namespace {

HRESULT name(HANDLE object, wchar_t (&text)[128]) noexcept {
    DWORD needed = 0;
    if (!object || !GetUserObjectInformationW(object, UOI_NAME, text, sizeof(text), &needed))
        return ss::dxgi::win32_failure(GetLastError());
    if (needed == 0 || needed > sizeof(text) || text[127] != 0)
        return E_UNEXPECTED;
    return S_OK;
}
HRESULT input(HANDLE object, BOOL &receives_input) noexcept {
    DWORD needed = 0;
    if (!object || !GetUserObjectInformationW(object, UOI_IO, &receives_input,
                                              sizeof(receives_input), &needed))
        return ss::dxgi::win32_failure(GetLastError());
    return needed == sizeof(receives_input) ? S_OK : E_UNEXPECTED;
}
void json_name(const wchar_t (&text)[128]) noexcept {
    std::putchar('"');
    for (size_t i = 0; i < 128 && text[i]; ++i)
        std::printf("\\u%04x", static_cast<unsigned>(text[i]));
    std::putchar('"');
}
} // namespace
int print_desktop_identity() noexcept {
    LARGE_INTEGER begin{}, end{}, frequency{};
    HRESULT clock = S_OK;
    if (!QueryPerformanceFrequency(&frequency))
        clock = ss::dxgi::win32_failure(GetLastError());
    else if (frequency.QuadPart <= 0)
        clock = E_UNEXPECTED;
    else if (!QueryPerformanceCounter(&begin))
        clock = ss::dxgi::win32_failure(GetLastError());
    else if (begin.QuadPart <= 0)
        clock = E_UNEXPECTED;
    const auto thread_id = GetCurrentThreadId();
    const auto process_id = GetCurrentProcessId();
    const auto station = GetProcessWindowStation();   // Borrowed, never closed here.
    const auto desktop = GetThreadDesktop(thread_id); // Borrowed, never closed here.
    const auto active = OpenInputDesktop(0, FALSE, DESKTOP_READOBJECTS); // Owned query handle.
    const auto open = active ? S_OK : ss::dxgi::win32_failure(GetLastError());
    wchar_t station_name[128]{}, desktop_name[128]{}, active_name[128]{};
    const auto station_hr = name(station, station_name);
    const auto desktop_hr = name(desktop, desktop_name);
    const auto active_hr = name(active, active_name);
    BOOL desktop_input = FALSE, active_input = FALSE;
    const auto desktop_io_hr = input(desktop, desktop_input);
    const auto active_io_hr = input(active, active_input);
    USEROBJECTFLAGS flags{};
    DWORD needed = 0;
    const auto flags_hr =
        station && GetUserObjectInformationW(station, UOI_FLAGS, &flags, sizeof(flags), &needed)
            ? S_OK
            : ss::dxgi::win32_failure(GetLastError());
    DWORD session = 0;
    const auto session_hr =
        ProcessIdToSessionId(process_id, &session) ? S_OK : ss::dxgi::win32_failure(GetLastError());
    const auto console = WTSGetActiveConsoleSessionId();
    const auto foreground = GetForegroundWindow();
    DWORD foreground_pid = 0, foreground_session = 0;
    const auto foreground_thread = GetWindowThreadProcessId(foreground, &foreground_pid);
    const auto foreground_hr =
        foreground_thread && ProcessIdToSessionId(foreground_pid, &foreground_session)
            ? S_OK
            : ss::dxgi::win32_failure(GetLastError());
    HRESULT close = S_FALSE;
    if (active)
        close = CloseDesktop(active) ? S_OK : ss::dxgi::win32_failure(GetLastError());
    if (!QueryPerformanceCounter(&end)) {
        if (SUCCEEDED(clock))
            clock = ss::dxgi::win32_failure(GetLastError());
    } else if (end.QuadPart <= 0 || end.QuadPart < begin.QuadPart) {
        if (SUCCEEDED(clock))
            clock = E_UNEXPECTED;
    }
    const bool same_name = SUCCEEDED(desktop_hr) && SUCCEEDED(active_hr) &&
                           std::wcscmp(desktop_name, active_name) == 0;
    const bool on_input = same_name && SUCCEEDED(desktop_io_hr) && SUCCEEDED(active_io_hr) &&
                          desktop_input && active_input;
    std::printf("{\"schema_version\":1,\"metadata_only\":true,\"pid\":%lu,\"thread_id\":%lu,"
                "\"qpc_frequency\":%lld,\"begin_qpc\":%lld,\"end_qpc\":%lld,\"clock_hresult\":%ld,"
                "\"station_name\":",
                process_id, thread_id, frequency.QuadPart, begin.QuadPart, end.QuadPart, clock);
    json_name(station_name);
    std::printf(",\"thread_desktop_name\":");
    json_name(desktop_name);
    std::printf(",\"input_desktop_name\":");
    json_name(active_name);
    std::printf(
        ",\"station_name_hresult\":%ld,\"thread_name_hresult\":%ld,\"input_name_hresult\":%ld,"
        "\"open_input_hresult\":%ld,\"close_input_hresult\":%ld,\"thread_input_hresult\":%ld,"
        "\"input_input_hresult\":%ld,\"thread_receives_input\":%s,\"input_receives_input\":%s,"
        "\"same_desktop_name_within_station\":%s,\"thread_on_input_desktop\":%s,"
        "\"station_flags_hresult\":%ld,\"station_flags\":%lu,\"station_visible\":%s,"
        "\"process_session_hresult\":%ld,\"process_session_id\":%lu,\"active_console_session_id\":%"
        "lu,"
        "\"console_session_available\":%s,\"process_on_active_console\":%s,"
        "\"foreground_hwnd\":%llu,\"foreground_thread_id\":%lu,\"foreground_pid\":%lu,"
        "\"foreground_session_hresult\":%ld,\"foreground_session_id\":%lu}\n",
        station_hr, desktop_hr, active_hr, open, close, desktop_io_hr, active_io_hr,
        desktop_input ? "true" : "false", active_input ? "true" : "false",
        same_name ? "true" : "false", on_input ? "true" : "false", flags_hr, flags.dwFlags,
        (flags.dwFlags & WSF_VISIBLE) ? "true" : "false", session_hr, session, console,
        console != 0xffffffff ? "true" : "false",
        SUCCEEDED(session_hr) && session == console && console != 0xffffffff ? "true" : "false",
        reinterpret_cast<unsigned long long>(foreground), foreground_thread, foreground_pid,
        foreground_hr, foreground_session);
    const HRESULT results[]{clock,    station_hr, desktop_hr,    active_hr,
                            open,     close,      desktop_io_hr, active_io_hr,
                            flags_hr, session_hr, foreground_hr};
    for (const auto hr : results)
        if (FAILED(hr))
            return 1;
    return 0;
}
