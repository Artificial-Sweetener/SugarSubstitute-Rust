// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_OWNED_CONNECTION_H_
#define SUGARSUBSTITUTE_OWNED_CONNECTION_H_

#include <windows.h>
#include <stdint.h>
#include "util/win/registration_protocol_win.h"

namespace sugarsubstitute {

// Replace only the external native boundary; the same owner drives fake tests.
struct ConnectionApi {
  decltype(&QueryPerformanceCounter) counter;
  decltype(&CloseHandle) close;
  decltype(&GetLastError) last_error;
  decltype(&SetLastError) set_error;
  decltype(&CancelIoEx) cancel;
  decltype(&WaitForSingleObject) wait;
  decltype(&GetOverlappedResult) result;
  decltype(&CreateFileW) open;
  decltype(&GetNamedPipeServerProcessId) server_id;
  decltype(&GetProcessId) process_id;
  decltype(&SetNamedPipeHandleState) mode;
  decltype(&CreateEventW) event;
  decltype(&TransactNamedPipe) transact;
  decltype(&GetProcessTimes) process_times;
};

// One original v2 cleanup cutoff, sticky after the first failure/expiry.
// The public v1 owner passes nullptr and retains its existing cleanup behavior.
struct CleanupLimit {
  int64_t start_counter = 0;
  int64_t total_counter = 0;
  int64_t previous_counter = 0;
  bool stopped = false;
};

enum class EventCustody { Owned, Borrowed };
enum class NativeCauseDomain : uint32_t { None = 0, Win32 = 1, Ntstatus = 2, Errno = 3 };

// One original deadline and durable OVERLAPPED/message owner. Issued buffers
// survive cancellation, unknown completion and failed checked closure.
struct OwnedConnection {
  HANDLE pipe = INVALID_HANDLE_VALUE;
  HANDLE event = nullptr;
  EventCustody event_custody = EventCustody::Owned;
  OVERLAPPED overlapped = {};
  crashpad::ClientToServerMessage request = {};
  crashpad::ServerToClientMessage response = {};
  int64_t deadline_counter = 0;
  int64_t frequency = 0;
  DWORD error = ERROR_SUCCESS;
  NativeCauseDomain error_domain = NativeCauseDomain::None;
  DWORD cleanup_error = ERROR_SUCCESS;
  DWORD cleanup_codes[8] = {};
  NativeCauseDomain cleanup_domains[8] = {};
  size_t cleanup_count = 0;
  uint64_t omitted_cleanup = 0;
  const ConnectionApi* boundary = nullptr;
  bool pending = false;

  const ConnectionApi& Api() const;
  bool Fail(DWORD cause);
  bool FailStatus(int32_t status);
  bool FailErrno(int cause);
  void RecordCleanup(DWORD cause);
  void RecordCleanupStatus(int32_t status);
  // Flooring preserves the exact original QPC deadline, including sub-ms tails.
  DWORD Remaining();
  bool CheckDeadline();
  bool ProcessBirth(HANDLE process, uint64_t* birth);
  bool CleanupAllowed(CleanupLimit* limit);
  bool Close(HANDLE* slot, HANDLE empty, CleanupLimit* limit);
  // Zero-wait cancellation/completion/checked-close; never closes a borrowed event.
  bool Retire(CleanupLimit* limit);
 private:
  void RememberCause(DWORD cause, NativeCauseDomain domain);
  void AppendCause(DWORD cause, NativeCauseDomain domain);
};

}  // namespace sugarsubstitute
#endif
