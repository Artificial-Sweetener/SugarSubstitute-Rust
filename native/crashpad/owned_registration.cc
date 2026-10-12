// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/owned_start.h"

#include <algorithm>
#include <limits>

namespace sugarsubstitute {
namespace {
const RegistrationApi system_api = {&QueryPerformanceCounter, &CloseHandle,
    &GetLastError, &SetLastError, &CancelIoEx, &WaitForSingleObject,
    &GetOverlappedResult, &CreateFileW, &WaitNamedPipeW,
    &GetNamedPipeServerProcessId, &GetProcessId, &SetNamedPipeHandleState,
    &CreateEventW, &TransactNamedPipe};
const RegistrationApi& Api(OwnedStart* owner) {
  return owner->api ? *owner->api : system_api;
}
bool Fail(OwnedStart* owner, DWORD error) {
  if (!owner->error) owner->error = error;
  Api(owner).set_error(error);
  return false;
}
bool Close(HANDLE* value, HANDLE empty, OwnedStart* owner) {
  if (*value == empty) return true;
  if (!Api(owner).close(*value)) {
    RecordCleanup(owner, Api(owner).last_error());
    return false;
  }
  *value = empty;
  return true;
}
DWORD Remaining(OwnedStart* owner) {
  LARGE_INTEGER now = {};
  if (!Api(owner).counter(&now)) {
    Fail(owner, Api(owner).last_error());
    return 0;
  }
  if (now.QuadPart < 0 || owner->frequency <= 0 ||
      owner->frequency > std::numeric_limits<int64_t>::max() / 1000) {
    Fail(owner, ERROR_INVALID_DATA);
    return 0;
  }
  if (now.QuadPart >= owner->deadline_counter) return 0;
  const uint64_t ticks = owner->deadline_counter - now.QuadPart;
  const uint64_t frequency = owner->frequency;
  if (ticks / frequency > 60) { Fail(owner, ERROR_INVALID_DATA); return 0; }
  // Divide before multiplying, with checked remainder bounds. Flooring cannot
  // extend the fixed original QPC deadline, including its final sub-millisecond.
  const uint64_t milliseconds = (ticks / frequency) * 1000 +
      ((ticks % frequency) * 1000) / frequency;
  return static_cast<DWORD>(std::min<uint64_t>(
      milliseconds, std::numeric_limits<DWORD>::max() - 1));
}
}  // namespace

void RecordCleanup(OwnedStart* owner, DWORD error) {
  if (!owner->cleanup_error) owner->cleanup_error = error;
  if (owner->cleanup_count < 8) owner->cleanup_codes[owner->cleanup_count++] = error;
  else if (owner->omitted_cleanup != std::numeric_limits<uint64_t>::max())
    ++owner->omitted_cleanup;
}
bool CheckDeadline(OwnedStart* owner) {
  return Remaining(owner) != 0 || Fail(owner, owner->error ? owner->error : ERROR_TIMEOUT);
}

bool RetireRegistration(OwnedStart* owner) {
  if (owner->pending) {
    // ERROR_NOT_FOUND permits querying completion but is not itself settlement.
    if (!Api(owner).cancel(owner->pipe, &owner->overlapped)) {
      const DWORD error = Api(owner).last_error();
      if (error != ERROR_NOT_FOUND) RecordCleanup(owner, error);
    }
    const DWORD completed = Api(owner).wait(owner->event, 0);
    if (completed != WAIT_OBJECT_0) {
      if (completed == WAIT_FAILED) RecordCleanup(owner, Api(owner).last_error());
      return false;
    }
    DWORD transferred = 0;
    if (!Api(owner).result(owner->pipe, &owner->overlapped, &transferred, FALSE)) {
      const DWORD error = Api(owner).last_error();
      if (error != ERROR_OPERATION_ABORTED) {
        if (error != ERROR_IO_INCOMPLETE) RecordCleanup(owner, error);
        return false;
      }
    }
    owner->pending = false;
  }
  const bool pipe_closed = Close(&owner->pipe, INVALID_HANDLE_VALUE, owner);
  const bool event_closed = Close(&owner->event, nullptr, owner);
  const bool initial_closed = Close(&owner->initial_pipe, nullptr, owner);
  const bool bootstrap_closed = Close(&owner->bootstrap_process, nullptr, owner);
  return pipe_closed && event_closed && initial_closed && bootstrap_closed;
}

bool ConfirmHandler(const std::wstring& name, OwnedStart* owner) {
  for (;;) {
    const DWORD remaining = Remaining(owner);
    if (!remaining) return Fail(owner, owner->error ? owner->error : ERROR_TIMEOUT);
    owner->pipe = Api(owner).open(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0,
        nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT |
        SECURITY_IDENTIFICATION, nullptr);
    if (owner->pipe != INVALID_HANDLE_VALUE) break;
    const DWORD error = Api(owner).last_error();
    if (error != ERROR_PIPE_BUSY) return Fail(owner, error);
    if (!Api(owner).pipe_wait(name.c_str(), remaining))
      return Fail(owner, Api(owner).last_error());
  }
  ULONG server_pid = 0;
  if (!Api(owner).server_id(owner->pipe, &server_pid))
    return Fail(owner, Api(owner).last_error());
  const DWORD handler_pid = Api(owner).process_id(owner->process);
  if (!handler_pid) return Fail(owner, Api(owner).last_error());
  if (server_pid != handler_pid) return Fail(owner, ERROR_ACCESS_DENIED);
  DWORD mode = PIPE_READMODE_MESSAGE;
  if (!Api(owner).mode(owner->pipe, &mode, nullptr, nullptr))
    return Fail(owner, Api(owner).last_error());
  owner->event = Api(owner).event(nullptr, TRUE, FALSE, nullptr);
  if (!owner->event) return Fail(owner, Api(owner).last_error());
  owner->overlapped.hEvent = owner->event;
  owner->request.type = crashpad::ClientToServerMessage::kPing;
  if (!CheckDeadline(owner)) return false;
  DWORD transferred = 0;
  if (!Api(owner).transact(owner->pipe, &owner->request, sizeof(owner->request),
      &owner->response, sizeof(owner->response), &transferred, &owner->overlapped)) {
    const DWORD error = Api(owner).last_error();
    if (error != ERROR_IO_PENDING) return Fail(owner, error);
    owner->pending = true;
    const DWORD remaining = Remaining(owner);
    const DWORD waited = Api(owner).wait(owner->event, remaining);
    if (waited != WAIT_OBJECT_0) {
      const DWORD cause = waited == WAIT_TIMEOUT ? ERROR_TIMEOUT : Api(owner).last_error();
      Fail(owner, cause);
      RetireRegistration(owner);
      return false;
    }
    if (!Api(owner).result(owner->pipe, &owner->overlapped, &transferred, FALSE)) {
      const DWORD cause = Api(owner).last_error();
      if (cause == ERROR_OPERATION_ABORTED) owner->pending = false;
      return Fail(owner, cause);
    }
    owner->pending = false;
  }
  if (transferred != sizeof(owner->response)) return Fail(owner, ERROR_INVALID_DATA);
  if (!CheckDeadline(owner)) return false;
  owner->completed = true;
  return RetireRegistration(owner);
}
}  // namespace sugarsubstitute
