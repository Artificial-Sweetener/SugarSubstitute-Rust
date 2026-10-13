// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Compile exact generated initialization/commit/installer and helper bodies.
// Fake only class declaration/storage and external OS/CRT/allocation boundaries.
#include "sugarsubstitute/owned_attach.h"
#include "sugarsubstitute/owned_attach_test_support.h"
#include <csignal>
#include <cerrno>
#include <new>
#include <string>

namespace {
struct RuntimeBoundary {
  DWORD error = 0;
  bool critical_ok = true, allocation_ok = true, vectored_ok = true, signal_ok = true;
  bool globals_ready = true;
  int critical_calls = 0, nothrow_calls = 0, normal_calls = 0;
  int filter_calls = 0, vector_calls = 0, signal_calls = 0;
  int early_callbacks = 0, debug_asserts = 0;
} native;
}
namespace base {
class Lock {
 public:
  static void* operator new(size_t, const std::nothrow_t&) noexcept;
  static void* operator new(size_t);
  static void operator delete(void*, const std::nothrow_t&) noexcept {}
  static void operator delete(void*) noexcept {}
};
}
namespace {
alignas(base::Lock) unsigned char lock_storage[sizeof(base::Lock)];
}
void* base::Lock::operator new(size_t, const std::nothrow_t&) noexcept {
  ++native.nothrow_calls; return native.allocation_ok ? lock_storage : nullptr;
}
void* base::Lock::operator new(size_t) { ++native.normal_calls; return lock_storage; }

namespace sugarsubstitute { struct OwnedStart; }
namespace crashpad {
using WinVMAddress = uint64_t;
template <typename To, typename From> To FromPointerCast(From* p) {
  return static_cast<To>(reinterpret_cast<uintptr_t>(p));
}
struct ExceptionStorage { uintptr_t exception_pointers = 0; DWORD thread_id = 0; };
ExceptionStorage g_crash_exception_information, g_non_crash_exception_information;
CRITICAL_SECTION g_critical_section_with_debug_info = {};
base::Lock* g_non_crash_dump_lock = nullptr;
HANDLE g_signal_exception = INVALID_HANDLE_VALUE;
WerRegistration g_wer_registration = {};
enum class StartupState { kNotReady, kSucceeded, kFailed };
StartupState startup = StartupState::kNotReady;
void SetHandlerStartupState(StartupState state) { startup = state; }
class CrashpadClient {
 public:
  sugarsubstitute::OwnedStart* owned_start_ = nullptr;
  sugarsubstitute::OwnedAttach* captured_attach_ = nullptr;
  std::wstring ipc_pipe_;
  struct VectoredOutput { PVOID value = nullptr; void reset(PVOID p) { value = p; } } vectored_handler_;
  bool InitializeCapturedAttach(sugarsubstitute::OwnedAttach*);
  bool CommitCapturedEvents(sugarsubstitute::OwnedAttach*);
  bool InstallCapturedHandlers(sugarsubstitute::OwnedAttach*);
  void RegisterHandlers();
};
}

namespace {
void ObserveGlobals() {
  ++native.early_callbacks;
  native.globals_ready &= crashpad::g_signal_exception == Token(10) &&
      crashpad::g_wer_registration.dump_without_crashing == Token(11) &&
      crashpad::g_wer_registration.dump_completed == Token(12) &&
      crashpad::startup == crashpad::StartupState::kSucceeded;
}
LONG WINAPI UnhandledExceptionHandler(EXCEPTION_POINTERS*) { ObserveGlobals(); return 0; }
LONG WINAPI HandleHeapCorruption(EXCEPTION_POINTERS*) { ObserveGlobals(); return 0; }
void HandleAbortSignal(int) { ObserveGlobals(); }
LPTOP_LEVEL_EXCEPTION_FILTER WINAPI FakeSetFilter(LPTOP_LEVEL_EXCEPTION_FILTER callback) {
  ++native.filter_calls; callback(nullptr); return nullptr;
}
PVOID WINAPI FakeAddVectored(ULONG, PVECTORED_EXCEPTION_HANDLER callback) {
  ++native.vector_calls;
  if (!native.vectored_ok) { native.error = ERROR_ACCESS_DENIED; return nullptr; }
  callback(nullptr); return Token(42);
}
using SignalHandler = void (*)(int);
SignalHandler FakeSignal(int, SignalHandler callback) {
  ++native.signal_calls;
  if (!native.signal_ok) { errno = EINVAL; return SIG_ERR; }
  callback(SIGABRT); return nullptr;
}
BOOL WINAPI FakeInitializeCriticalSectionEx(CRITICAL_SECTION* section, DWORD, DWORD) {
  ++native.critical_calls;
  section->DebugInfo = reinterpret_cast<PRTL_CRITICAL_SECTION_DEBUG>(77);
  native.error = ERROR_NO_SYSTEM_RESOURCES;
  return native.critical_ok;
}
void WINAPI FakeInitializeCriticalSection(CRITICAL_SECTION* section) {
  ++native.critical_calls; section->DebugInfo = reinterpret_cast<PRTL_CRITICAL_SECTION_DEBUG>(77);
}
DWORD WINAPI FakeVersion() { return 10; }
DWORD WINAPI RuntimeLastError() { return native.error; }
void WINAPI RuntimeSetError(DWORD code) { native.error = code; }
struct LoggingBoundary {
  template <typename T> LoggingBoundary& operator<<(const T&) { return *this; }
  ~LoggingBoundary() { native.error = ERROR_PRIVILEGE_NOT_HELD; }
};
}
#define GET_FUNCTION_REQUIRED(module, function) (&FakeInitializeCriticalSectionEx)
#define GetVersion FakeVersion
#define GetLastError RuntimeLastError
#define SetLastError RuntimeSetError
#define InitializeCriticalSection FakeInitializeCriticalSection
#define SetUnhandledExceptionFilter FakeSetFilter
#define AddVectoredExceptionHandler FakeAddVectored
#define signal FakeSignal
#define PLOG(level) LoggingBoundary()
#define DCHECK_NE(left, right) do { if ((left) == (right)) ++native.debug_asserts; } while (false)
namespace crashpad {
#include "capture_runtime_helper.inc"
namespace {
#include "capture_runtime_common.inc"
}
#include "capture_runtime_methods.inc"
}
#undef GetVersion
#undef GetLastError
#undef SetLastError
#undef InitializeCriticalSection
#undef SetUnhandledExceptionFilter
#undef AddVectoredExceptionHandler
#undef signal

namespace {
struct Fixture {
  OwnedAttach owner;
  crashpad::CrashpadClient client;
  Fixture() {
    boundary = {}; native = {};
    owner.connection.boundary = &api;
    crashpad::g_critical_section_with_debug_info = {};
    crashpad::g_non_crash_dump_lock = nullptr;
    crashpad::g_signal_exception = INVALID_HANDLE_VALUE;
    crashpad::g_wer_registration = {};
    crashpad::g_wer_registration.dump_without_crashing = INVALID_HANDLE_VALUE;
    crashpad::g_wer_registration.dump_completed = INVALID_HANDLE_VALUE;
    crashpad::startup = crashpad::StartupState::kNotReady;
  }
  bool Init() { return client.InitializeCapturedAttach(&owner); }
  bool Commit() {
    for (size_t i = 0; i < 3; ++i) {
      owner.response_events[i] = Token(10 + i);
      owner.response_custody[i] = sugarsubstitute::ResponseEvent::Owned;
    }
    return client.CommitCapturedEvents(&owner);
  }
};
bool OriginalHelperCauseSurvivesLogger() {
  Fixture f; native.critical_ok = false;
  const bool accepted = f.Init();
  return !accepted && f.client.captured_attach_ == &f.owner &&
      f.owner.connection.error == ERROR_NO_SYSTEM_RESOURCES &&
      f.owner.connection.error_domain == sugarsubstitute::NativeCauseDomain::Win32 &&
      native.error == ERROR_NO_SYSTEM_RESOURCES && native.critical_calls == 1 &&
      !native.nothrow_calls && !native.normal_calls && !native.filter_calls &&
      !f.owner.connection.request.registration.client_process_id &&
      crashpad::g_critical_section_with_debug_info.DebugInfo && !crashpad::g_non_crash_dump_lock;
}
bool CheckedCapturedOomRetainsGlobalPartial() {
  Fixture f; native.allocation_ok = false;
  return !f.Init() && f.client.captured_attach_ == &f.owner &&
      f.owner.connection.error == ERROR_NOT_ENOUGH_MEMORY && native.nothrow_calls == 1 &&
      !native.normal_calls && !native.filter_calls &&
      crashpad::g_critical_section_with_debug_info.DebugInfo && !crashpad::g_non_crash_dump_lock;
}
bool ActualRegistrationAddresses() {
  Fixture f; if (!f.Init()) return false;
  const auto& request = f.owner.connection.request.registration;
  return request.version == crashpad::RegistrationRequest::kMessageVersion &&
      request.client_process_id == GetCurrentProcessId() &&
      request.crash_exception_information == reinterpret_cast<uintptr_t>(&crashpad::g_crash_exception_information) &&
      request.non_crash_exception_information == reinterpret_cast<uintptr_t>(&crashpad::g_non_crash_exception_information) &&
      request.critical_section_address == reinterpret_cast<uintptr_t>(&crashpad::g_critical_section_with_debug_info) &&
      native.nothrow_calls == 1 && !native.filter_calls;
}
bool ActualGlobalsBeforeFirstHandlerCallback() {
  Fixture f; if (!f.Init() || !f.Commit()) return false;
  if (!f.client.InstallCapturedHandlers(&f.owner)) return false;
  for (auto custody : f.owner.response_custody)
    if (custody != sugarsubstitute::ResponseEvent::CallbackPinned) return false;
  return f.owner.globals_committed && native.globals_ready && native.early_callbacks == 3 &&
      native.filter_calls == 1 && native.vector_calls == 1 && native.signal_calls == 1;
}
bool PartialHandlersRetainFirstAndErrnoSecondary() {
  Fixture f; if (!f.Init() || !f.Commit()) return false;
  native.vectored_ok = native.signal_ok = false;
  if (f.client.InstallCapturedHandlers(&f.owner)) return false;
  return f.owner.connection.error == ERROR_ACCESS_DENIED &&
      f.owner.connection.error_domain == sugarsubstitute::NativeCauseDomain::Win32 &&
      f.owner.connection.cleanup_count == 1 && f.owner.connection.cleanup_codes[0] == EINVAL &&
      f.owner.connection.cleanup_domains[0] == sugarsubstitute::NativeCauseDomain::Errno &&
      f.owner.globals_committed && crashpad::g_signal_exception == Token(10) &&
      native.filter_calls == 1 && native.vector_calls == 1 && native.signal_calls == 1 &&
      native.globals_ready && !native.debug_asserts;
}
bool ErrnoPrimaryRetained() {
  Fixture f; if (!f.Init() || !f.Commit()) return false;
  native.signal_ok = false;
  return !f.client.InstallCapturedHandlers(&f.owner) &&
      f.owner.connection.error == EINVAL &&
      f.owner.connection.error_domain == sugarsubstitute::NativeCauseDomain::Errno &&
      !native.debug_asserts && f.owner.globals_committed;
}
bool UnknownEventCannotCommitOrInstall() {
  Fixture f; if (!f.Init()) return false;
  f.owner.response_events[0] = Token(10);
  return !f.client.CommitCapturedEvents(&f.owner) && !f.owner.globals_committed &&
      crashpad::g_signal_exception == INVALID_HANDLE_VALUE && !native.filter_calls;
}
bool LegacyHelperFailureBehaviorPreserved() {
  Fixture f; native.critical_ok = false;
  return crashpad::CommonInProcessInitialization() && native.normal_calls == 1 &&
      !native.nothrow_calls && crashpad::g_non_crash_dump_lock && !f.owner.connection.error;
}
bool LegacySignalDcheckBehaviorPreserved() {
  Fixture f; native.signal_ok = false;
  f.client.RegisterHandlers();
  return native.signal_calls == 1 && native.debug_asserts == 1 && !f.owner.connection.error;
}
}
int main() {
  struct Case { const char* name; bool (*run)(); } cases[] = {
    {"helper-original-cause-before-lock-survives-logger", &OriginalHelperCauseSurvivesLogger},
    {"checked-captured-oom-retains-partial-globals", &CheckedCapturedOomRetainsGlobalPartial},
    {"actual-message-original-global-addresses", &ActualRegistrationAddresses},
    {"all-three-globals-pinned-before-first-handler", &ActualGlobalsBeforeFirstHandlerCallback},
    {"partial-handlers-first-win32-and-secondary-errno", &PartialHandlersRetainFirstAndErrnoSecondary},
    {"crt-primary-domain-retained", &ErrnoPrimaryRetained},
    {"unknown-events-no-global-commit", &UnknownEventCannotCommitOrInstall},
    {"legacy-helper-failure-behavior-preserved", &LegacyHelperFailureBehaviorPreserved},
    {"legacy-signal-dcheck-preserved", &LegacySignalDcheckBehaviorPreserved},
  };
  for (const auto& test : cases)
    if (!test.run()) { std::fprintf(stderr, "FAIL: %s\n", test.name); return 1; }
  std::puts("9 exact generated Crashpad runtime/helper CPU boundary cases PASS; no native campaign");
}
