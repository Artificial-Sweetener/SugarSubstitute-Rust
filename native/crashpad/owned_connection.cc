// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/owned_connection.h"

#include <algorithm>
#include <limits>

namespace sugarsubstitute {
namespace {
const ConnectionApi system_api = {&QueryPerformanceCounter, &CloseHandle,
    &GetLastError, &SetLastError, &CancelIoEx, &WaitForSingleObject,
    &GetOverlappedResult, &CreateFileW, &GetNamedPipeServerProcessId,
    &GetProcessId, &SetNamedPipeHandleState, &CreateEventW, &TransactNamedPipe,
    &GetProcessTimes};
}  // namespace

const ConnectionApi& OwnedConnection::Api() const {
  return boundary ? *boundary : system_api;
}
void OwnedConnection::RememberCause(DWORD cause, NativeCauseDomain domain) {
  if (!error) { error = cause; error_domain = domain; }
  else AppendCause(cause, domain);
}
bool OwnedConnection::Fail(DWORD cause) {
  if (!cause) cause = ERROR_GEN_FAILURE;
  RememberCause(cause, NativeCauseDomain::Win32);
  Api().set_error(cause);
  return false;
}
bool OwnedConnection::FailStatus(int32_t status) {
  if (status >= 0) return Fail(ERROR_INVALID_DATA);
  RememberCause(static_cast<DWORD>(status), NativeCauseDomain::Ntstatus);
  Api().set_error(ERROR_GEN_FAILURE);
  return false;
}
bool OwnedConnection::FailErrno(int cause) {
  if (cause <= 0) return Fail(ERROR_INVALID_DATA);
  RememberCause(static_cast<DWORD>(cause), NativeCauseDomain::Errno);
  Api().set_error(ERROR_GEN_FAILURE);
  return false;
}
bool OwnedConnection::ProcessBirth(HANDLE process, uint64_t* birth) {
  FILETIME creation = {}, exit = {}, kernel = {}, user = {};
  if (!Api().process_times(process, &creation, &exit, &kernel, &user))
    return Fail(Api().last_error());
  const uint64_t actual = (uint64_t{creation.dwHighDateTime} << 32) |
      creation.dwLowDateTime;
  if (!actual) return Fail(ERROR_INVALID_DATA);
  *birth = actual;
  return true;
}
void OwnedConnection::AppendCause(DWORD cause, NativeCauseDomain domain) {
  if (!cleanup_error) cleanup_error = cause;
  if (cleanup_count < 8) {
    cleanup_codes[cleanup_count] = cause;
    cleanup_domains[cleanup_count++] = domain;
  } else if (omitted_cleanup != std::numeric_limits<uint64_t>::max()) ++omitted_cleanup;
}
void OwnedConnection::RecordCleanup(DWORD cause) {
  AppendCause(cause ? cause : ERROR_GEN_FAILURE, NativeCauseDomain::Win32);
}
void OwnedConnection::RecordCleanupStatus(int32_t status) {
  if (status >= 0) { RecordCleanup(ERROR_INVALID_DATA); return; }
  AppendCause(static_cast<DWORD>(status), NativeCauseDomain::Ntstatus);
}
DWORD OwnedConnection::Remaining() {
  LARGE_INTEGER now = {};
  if (!Api().counter(&now)) {
    Fail(Api().last_error());
    return 0;
  }
  if (now.QuadPart < 0 || frequency <= 0 ||
      frequency > std::numeric_limits<int64_t>::max() / 1000) {
    Fail(ERROR_INVALID_DATA);
    return 0;
  }
  if (now.QuadPart >= deadline_counter) return 0;
  const uint64_t ticks = deadline_counter - now.QuadPart;
  const uint64_t rate = frequency;
  if (ticks / rate > 60) { Fail(ERROR_INVALID_DATA); return 0; }
  const uint64_t milliseconds = (ticks / rate) * 1000 +
      ((ticks % rate) * 1000) / rate;
  return static_cast<DWORD>(std::min<uint64_t>(
      milliseconds, std::numeric_limits<DWORD>::max() - 1));
}
bool OwnedConnection::CheckDeadline() {
  if (error) return false;
  if (Remaining() != 0) return true;
  if (!error) return Fail(ERROR_TIMEOUT);
  return false;
}
bool OwnedConnection::CleanupAllowed(CleanupLimit* limit) {
  if (!limit) return true;
  if (limit->stopped) return false;
  LARGE_INTEGER now = {};
  DWORD failure = ERROR_SUCCESS;
  if (limit->start_counter < 0 || limit->total_counter <= limit->start_counter)
    failure = ERROR_INVALID_DATA;
  else if (!Api().counter(&now)) failure = Api().last_error();
  else if (now.QuadPart < limit->start_counter ||
           now.QuadPart < limit->previous_counter) failure = ERROR_INVALID_DATA;
  else if (now.QuadPart >= limit->total_counter) failure = ERROR_TIMEOUT;
  else { limit->previous_counter = now.QuadPart; return true; }
  limit->stopped = true;
  RecordCleanup(failure);
  return false;
}
bool OwnedConnection::Close(HANDLE* slot, HANDLE empty, CleanupLimit* limit) {
  if (*slot == empty) return true;
  if (!CleanupAllowed(limit)) return false;
  if (!Api().close(*slot)) {
    RecordCleanup(Api().last_error());
    return false;
  }
  *slot = empty;
  return true;
}
bool OwnedConnection::Retire(CleanupLimit* limit) {
  if (pending) {
    if (!CleanupAllowed(limit)) return false;
    // ERROR_NOT_FOUND is a cancel race, never proof of completion.
    if (!Api().cancel(pipe, &overlapped)) {
      const DWORD cause = Api().last_error();
      if (cause != ERROR_NOT_FOUND) RecordCleanup(cause);
    }
    if (!CleanupAllowed(limit)) return false;
    const DWORD completed = Api().wait(event, 0);
    if (completed != WAIT_OBJECT_0) {
      if (completed == WAIT_FAILED) RecordCleanup(Api().last_error());
      return false;
    }
    if (!CleanupAllowed(limit)) return false;
    DWORD transferred = 0;
    if (!Api().result(pipe, &overlapped, &transferred, FALSE)) {
      const DWORD cause = Api().last_error();
      if (cause != ERROR_OPERATION_ABORTED) {
        if (cause != ERROR_IO_INCOMPLETE) RecordCleanup(cause);
        return false;
      }
    }
    pending = false;
  }
  const bool pipe_closed = Close(&pipe, INVALID_HANDLE_VALUE, limit);
  const bool event_closed = event_custody == EventCustody::Borrowed ||
      Close(&event, nullptr, limit);
  if (event_custody == EventCustody::Borrowed) event = nullptr;
  return pipe_closed && event_closed;
}
}  // namespace sugarsubstitute
