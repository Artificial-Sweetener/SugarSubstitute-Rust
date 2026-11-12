// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Exercise the actual readiness publisher and main gate. Only external clocks
// and the acquisition boundary are replaced; no native acquisition or GUI runs.
#include "descriptor_update.h"
#include "desktop_identity.h"
#include "format_acquisition.h"
#include "native_error.h"
#include <Windows.h>
#include <array>
#include <cerrno>
#include <cstdio>
#include <cstring>
#include <cwchar>
#include <d3d11.h>
#include <dxgi1_6.h>
#include <fcntl.h>
#include <io.h>
#include <stdexcept>
#include <string>
#include <wrl/client.h>

namespace {
enum class ClockCase { Positive, FrequencyFalse, FrequencyZero, CounterFalse, CounterZero };
ClockCase clock_case = ClockCase::Positive;
unsigned frequency_calls = 0, counter_calls = 0, acquisition_calls = 0;
bool cleanup_failed = false;
BOOL WINAPI fixture_frequency(LARGE_INTEGER *value) noexcept {
    ++frequency_calls;
    value->QuadPart = clock_case == ClockCase::FrequencyZero ? 0 : 10000000;
    if (clock_case != ClockCase::FrequencyFalse)
        return TRUE;
    SetLastError(0);
    return FALSE;
}
BOOL WINAPI fixture_counter(LARGE_INTEGER *value) noexcept {
    ++counter_calls;
    value->QuadPart = clock_case == ClockCase::CounterZero ? 0 : 101;
    if (clock_case != ClockCase::CounterFalse)
        return TRUE;
    SetLastError(0);
    return FALSE;
}
void fixture_acquisition(IDXGIOutputDuplication &, UINT, FormatAcquisition &) noexcept {
    ++acquisition_calls;
}
void check(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
} // namespace

// Windows/stdio declarations are already included. These substitutions affect
// only the authored diagnostic callsites; standard namespaces are unchanged.
#define QueryPerformanceFrequency fixture_frequency
#define QueryPerformanceCounter fixture_counter
#define inspect_next_format fixture_acquisition
#define main descriptor_program_main
#include "descriptor_probe.cpp"
#undef main
#undef inspect_next_format
#undef QueryPerformanceCounter
#undef QueryPerformanceFrequency

namespace {
class OwnedStdout {
    std::array<wchar_t, MAX_PATH> path_{};
    std::array<wchar_t, MAX_PATH> failure_path_{};
    std::array<char, 4096> buffer_{};
    int saved_ = -1;
    bool file_created_ = false;
    bool failure_file_created_ = false;

    void restore() noexcept {
        if (saved_ >= 0) {
            // Reopening discards failed output and reconstructs a possibly closed
            // FILE. Configure the new stream before any I/O or descriptor transfer.
            FILE *fresh = nullptr;
            const auto reopened = _wfreopen_s(&fresh, path_.data(), L"w+b", stdout);
            if (reopened != 0 || fresh != stdout) {
                std::fprintf(stderr, "Readiness fixture restore freopen failed: %d\n", reopened);
                cleanup_failed = true;
            } else {
                if (std::setvbuf(stdout, nullptr, _IONBF, 0) != 0) {
                    const auto cause = errno;
                    std::fprintf(stderr, "Readiness fixture restore setvbuf failed: %d\n", cause);
                    cleanup_failed = true;
                }
                if (_dup2(saved_, _fileno(stdout)) != 0) {
                    const auto cause = errno;
                    std::fprintf(stderr, "Readiness fixture restore dup2 failed: %d\n", cause);
                    cleanup_failed = true;
                }
            }
            const auto closed = _close(saved_);
            const auto close_cause = errno;
            saved_ = -1;
            if (closed != 0) {
                std::fprintf(stderr, "Readiness fixture saved descriptor close failed: %d\n",
                             close_cause);
                cleanup_failed = true;
            }
        }
        if (file_created_) {
            if (!DeleteFileW(path_.data())) {
                const auto cause = GetLastError();
                std::fprintf(stderr, "Readiness fixture temporary-file deletion failed: %lu\n",
                             cause);
                cleanup_failed = true;
            }
            file_created_ = false;
        }
        if (failure_file_created_) {
            if (!DeleteFileW(failure_path_.data())) {
                const auto cause = GetLastError();
                std::fprintf(stderr, "Readiness fixture read-only-file deletion failed: %lu\n",
                             cause);
                cleanup_failed = true;
            }
            failure_file_created_ = false;
        }
    }

  public:
    explicit OwnedStdout(bool reject_redirection = false) {
        try {
            std::array<wchar_t, MAX_PATH> directory{}, resolved{};
            const auto length =
                GetTempPathW(static_cast<DWORD>(directory.size()), directory.data());
            check(length > 0 && length < directory.size(), "Temporary directory is unavailable");
            const auto handle =
                CreateFileW(directory.data(), FILE_READ_ATTRIBUTES,
                            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                            OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, nullptr);
            check(handle != INVALID_HANDLE_VALUE, "Temporary directory cannot be inspected");
            const auto count = GetFinalPathNameByHandleW(handle, resolved.data(),
                                                         static_cast<DWORD>(resolved.size()), 0);
            const bool closed = CloseHandle(handle) != FALSE;
            check(closed && count > 0 && count < resolved.size() &&
                      std::wcsncmp(resolved.data(), L"\\\\?\\D:\\", 7) == 0,
                  "Temporary directory must resolve to D:");
            file_created_ = GetTempFileNameW(directory.data(), L"rdy", 0, path_.data()) != 0;
            check(file_created_, "Owned output file could not be created");
            check(std::fflush(stdout) == 0, "Original stdout flush failed");
            saved_ = _dup(_fileno(stdout));
            check(saved_ >= 0, "Original stdout duplication failed");
            FILE *redirected = nullptr;
            // A regular owned file cannot be a directory. This valid external
            // pathname forces an open failure that closes the previous FILE.
            const auto rejected_path = std::wstring(path_.data()) + L"\\unavailable";
            const auto redirection =
                _wfreopen_s(&redirected, reject_redirection ? rejected_path.c_str() : path_.data(),
                            L"w+b", stdout);
            if (redirection != 0 || redirected != stdout)
                throw std::runtime_error("Owned stdout redirection failed: " +
                                         std::to_string(redirection));
            check(std::setvbuf(stdout, buffer_.data(), _IOFBF, buffer_.size()) == 0,
                  "Owned stdout buffering failed");
        } catch (...) {
            restore();
            throw;
        }
    }
    OwnedStdout(const OwnedStdout &) = delete;
    OwnedStdout &operator=(const OwnedStdout &) = delete;
    ~OwnedStdout() {
        // Restore even when a test assertion unwinds, before buffer_ is destroyed.
        restore();
    }
    void close_output(bool unbuffered) {
        if (unbuffered)
            check(std::setvbuf(stdout, nullptr, _IONBF, 0) == 0,
                  "Unbuffered failure boundary could not be configured");
        const std::wstring captured_path(path_.data());
        const auto directory = captured_path.substr(0, captured_path.find_last_of(L'\\') + 1);
        failure_file_created_ =
            GetTempFileNameW(directory.c_str(), L"rdo", 0, failure_path_.data()) != 0;
        check(failure_file_created_, "Read-only stdout file could not be created");
        const auto handle = CreateFileW(failure_path_.data(), GENERIC_READ,
                                        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                                        nullptr, OPEN_EXISTING, 0, nullptr);
        check(handle != INVALID_HANDLE_VALUE, "Read-only stdout resource could not be opened");
        const auto descriptor = _open_osfhandle(reinterpret_cast<intptr_t>(handle), _O_RDONLY);
        if (descriptor < 0) {
            check(CloseHandle(handle) != FALSE, "Rejected read-only handle cleanup failed");
            throw std::runtime_error("Read-only handle could not enter the CRT boundary");
        }
        const bool redirected = _dup2(descriptor, _fileno(stdout)) == 0;
        const bool closed = _close(descriptor) == 0;
        check(redirected && closed, "Read-only stdout resource transfer failed");
    }
    std::string contents() {
        check(std::fflush(stdout) == 0 && std::fseek(stdout, 0, SEEK_SET) == 0,
              "Owned output could not be observed");
        std::array<char, 4096> data{};
        const auto count = std::fread(data.data(), 1, data.size(), stdout);
        check(count < data.size() && !std::ferror(stdout), "Readiness output exceeded its bound");
        return {data.data(), count};
    }
};
void reset(ClockCase mode) {
    check(!cleanup_failed, "Earlier stdout restoration failed");
    clock_case = mode;
    frequency_calls = counter_calls = acquisition_calls = 0;
}
int rejected_program() {
    char executable[] = "descriptor_probe";
    char argument[] = "--acquire-format-pointer-trigger-once";
    char *arguments[]{executable, argument};
    return descriptor_program_main(2, arguments);
}
void positive_wire_is_exact_and_bounded() {
    reset(ClockCase::Positive);
    OwnedStdout output;
    const auto monitor = reinterpret_cast<HMONITOR>(static_cast<uintptr_t>(123));
    check(publish_acquisition_readiness(monitor), "Positive readiness publication failed");
    const auto observed = output.contents();
    std::array<char, 512> expected{};
    const auto length = std::snprintf(
        expected.data(), expected.size(),
        "{\"schema_version\":1,\"event\":\"format_acquisition_ready\",\"pid\":%lu,\"thread\":%lu,"
        "\"monitor\":123,\"qpc_frequency\":10000000,\"ready_qpc\":101}\n",
        GetCurrentProcessId(), GetCurrentThreadId());
    check(length > 0 && static_cast<size_t>(length) < expected.size() &&
              GetCurrentProcessId() > 0 && GetCurrentThreadId() > 0 &&
              observed == std::string(expected.data(), static_cast<size_t>(length)) &&
              frequency_calls == 1 && counter_calls == 1 && acquisition_calls == 0,
          "Readiness wire keys/identity/clock/bound differ");
}
void rejected_clocks_never_acquire(ClockCase mode) {
    reset(mode);
    OwnedStdout output;
    const auto expected_counter_calls =
        mode == ClockCase::FrequencyFalse || mode == ClockCase::FrequencyZero ? 0u : 1u;
    check(rejected_program() == 1 && frequency_calls == 1 &&
              counter_calls == expected_counter_calls && acquisition_calls == 0,
          "Rejected clock publication bypassed the actual acquisition gate");
    const auto observed = output.contents();
    check(observed.find("format_acquisition_ready") == std::string::npos &&
              observed.find("\"attempted\":false") != std::string::npos,
          "Rejected clock issued readiness or acquisition metadata");
}
void failed_output_never_acquires(bool unbuffered) {
    reset(ClockCase::Positive);
    OwnedStdout output;
    output.close_output(unbuffered);
    check(rejected_program() == 1 && frequency_calls == 1 && counter_calls == 1 &&
              acquisition_calls == 0 && std::ferror(stdout),
          "Failed real stdout publication bypassed acquisition rejection");
}
void failed_output_never_escapes_restoration() {
    reset(ClockCase::Positive);
    OwnedStdout outer;
    failed_output_never_acquires(false);
    check(!cleanup_failed && outer.contents().empty(),
          "Failed readiness/final bytes escaped into the restored outer stream");
}
void partial_redirection_failure_reconstructs_live_stdout() {
    reset(ClockCase::Positive);
    OwnedStdout outer;
    bool rejected = false;
    try {
        OwnedStdout invalid(true);
    } catch (const std::runtime_error &cause) {
        rejected = std::string(cause.what()).starts_with("Owned stdout redirection failed: ");
    }
    check(rejected && !cleanup_failed && _fileno(stdout) >= 0 &&
              std::fputs("recovered\n", stdout) >= 0 && outer.contents() == "recovered\n" &&
              acquisition_calls == 0,
          "Partial redirection failure left closed stdout or leaked output/acquisition");
}
} // namespace
int main() {
    try {
        positive_wire_is_exact_and_bounded();
        rejected_clocks_never_acquire(ClockCase::FrequencyFalse);
        rejected_clocks_never_acquire(ClockCase::FrequencyZero);
        rejected_clocks_never_acquire(ClockCase::CounterFalse);
        rejected_clocks_never_acquire(ClockCase::CounterZero);
        failed_output_never_acquires(true);
        failed_output_never_acquires(false);
        failed_output_never_escapes_restoration();
        partial_redirection_failure_reconstructs_live_stdout();
        check(!cleanup_failed, "Owned test cleanup failed");
        std::puts("9 readiness tests passed (actual publisher/gate; no acquisition/GUI)");
        return 0;
    } catch (const std::exception &error) {
        std::fprintf(stderr, "Readiness test failed: %s\n", error.what());
        return 1;
    }
}
