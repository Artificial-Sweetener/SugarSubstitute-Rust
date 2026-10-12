// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_OWNED_START_H_
#define SUGARSUBSTITUTE_OWNED_START_H_

#include <windows.h>
#include <stdint.h>
#include <string>
#include "util/win/registration_protocol_win.h"

namespace sugarsubstitute {

// Private replaceable external boundary for deterministic tests of actual owner
// transitions. Production uses the exact Windows functions, never fake facts.
struct RegistrationApi {
  decltype(&QueryPerformanceCounter) counter;
  decltype(&CloseHandle) close;
  decltype(&GetLastError) last_error;
  decltype(&SetLastError) set_error;
  decltype(&CancelIoEx) cancel;
  decltype(&WaitForSingleObject) wait;
  decltype(&GetOverlappedResult) result;
  decltype(&CreateFileW) open;
  decltype(&WaitNamedPipeW) pipe_wait;
  decltype(&GetNamedPipeServerProcessId) server_id;
  decltype(&GetProcessId) process_id;
  decltype(&SetNamedPipeHandleState) mode;
  decltype(&CreateEventW) event;
  decltype(&TransactNamedPipe) transact;
};

// One process-lifetime registration owner. Pending OVERLAPPED storage is never
// destroyed on cancellation; subsequent finite retirement calls observe it.
struct OwnedStart {
  HANDLE process = nullptr;
  HANDLE thread = nullptr;
  HANDLE initial_pipe = nullptr;
  HANDLE bootstrap_process = nullptr;
  HANDLE pipe = INVALID_HANDLE_VALUE;
  HANDLE event = nullptr;
  OVERLAPPED overlapped = {};
  crashpad::ClientToServerMessage request = {};
  crashpad::ServerToClientMessage response = {};
  int64_t deadline_counter = 0;
  int64_t frequency = 0;
  DWORD error = ERROR_SUCCESS;
  DWORD cleanup_error = ERROR_SUCCESS;
  DWORD cleanup_codes[8] = {};
  size_t cleanup_count = 0;
  uint64_t omitted_cleanup = 0;
  const RegistrationApi* api = nullptr;
  bool pending = false;
  bool completed = false;
};

bool ConfirmHandler(const std::wstring& name, OwnedStart* owner);
// Returns false while actual cancellation completion/checked closure is pending.
bool RetireRegistration(OwnedStart* owner);
bool CheckDeadline(OwnedStart* owner);
void RecordCleanup(OwnedStart* owner, DWORD error);

}  // namespace sugarsubstitute
#endif
