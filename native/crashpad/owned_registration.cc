// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/owned_start.h"

namespace sugarsubstitute {
bool RetireRegistration(OwnedStart* owner) {
  const bool connection_closed = owner->connection.Retire(nullptr);
  if (owner->connection.pending) return false;
  const bool initial_closed = owner->connection.Close(&owner->initial_pipe, nullptr, nullptr);
  const bool bootstrap_closed = owner->connection.Close(&owner->bootstrap_process, nullptr, nullptr);
  return connection_closed && initial_closed && bootstrap_closed;
}

bool PrepareInheritedRegistration(const std::wstring& name, OwnedStart* owner) {
  if (owner->prepared || owner->process || !owner->initial_pipe ||
      !owner->bootstrap_process || owner->connection.pipe != INVALID_HANDLE_VALUE)
    return owner->connection.Fail(ERROR_INVALID_STATE);
  if (!owner->connection.CheckDeadline()) return false;
  ULONG initial_pid = 0;
  if (!owner->connection.Api().server_id(owner->initial_pipe, &initial_pid))
    return owner->connection.Fail(owner->connection.Api().last_error());
  owner->creator_pid = owner->connection.Api().process_id(owner->bootstrap_process);
  if (!owner->creator_pid) return owner->connection.Fail(owner->connection.Api().last_error());
  if (initial_pid != owner->creator_pid) return owner->connection.Fail(ERROR_ACCESS_DENIED);
  if (!owner->connection.ProcessBirth(owner->bootstrap_process, &owner->creator_birth))
    return false;
  // Connect before inheritance while the creator retains the first instance.
  // Confirmation uses this exact endpoint, never a later instance by name.
  owner->connection.pipe = owner->connection.Api().open(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0,
      nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT |
      SECURITY_IDENTIFICATION, nullptr);
  if (owner->connection.pipe == INVALID_HANDLE_VALUE)
    return owner->connection.Fail(owner->connection.Api().last_error());
  ULONG server_pid = 0;
  if (!owner->connection.Api().server_id(owner->connection.pipe, &server_pid))
    return owner->connection.Fail(owner->connection.Api().last_error());
  if (server_pid != owner->creator_pid) return owner->connection.Fail(ERROR_ACCESS_DENIED);
  DWORD mode = PIPE_READMODE_MESSAGE;
  if (!owner->connection.Api().mode(owner->connection.pipe, &mode, nullptr, nullptr))
    return owner->connection.Fail(owner->connection.Api().last_error());
  if (!owner->connection.CheckDeadline()) return false;
  owner->prepared = true;
  return true;
}

bool ConfirmHandler(OwnedStart* owner) {
  if (!owner->prepared || owner->connection.pipe == INVALID_HANDLE_VALUE ||
      !owner->initial_pipe || !owner->process || owner->completed || owner->connection.pending)
    return owner->connection.Fail(ERROR_INVALID_STATE);
  if (!owner->connection.CheckDeadline()) return false;
  const DWORD handler_pid = owner->connection.Api().process_id(owner->process);
  if (!handler_pid) return owner->connection.Fail(owner->connection.Api().last_error());
  if (handler_pid == owner->creator_pid) return owner->connection.Fail(ERROR_ACCESS_DENIED);
  if (!owner->connection.ProcessBirth(owner->process, &owner->handler_birth)) return false;
  owner->handler_pid = handler_pid;
  const DWORD before = owner->connection.Api().wait(owner->process, 0);
  if (before != WAIT_TIMEOUT)
    return owner->connection.Fail(before == WAIT_FAILED ? owner->connection.Api().last_error() : ERROR_PROCESS_ABORTED);
  owner->connection.event = owner->connection.Api().event(nullptr, TRUE, FALSE, nullptr);
  if (!owner->connection.event) return owner->connection.Fail(owner->connection.Api().last_error());
  owner->connection.overlapped.hEvent = owner->connection.event;
  owner->connection.request.type = crashpad::ClientToServerMessage::kPing;
  if (!owner->connection.CheckDeadline()) return false;
  DWORD transferred = 0;
  if (!owner->connection.Api().transact(owner->connection.pipe, &owner->connection.request, sizeof(owner->connection.request),
      &owner->connection.response, sizeof(owner->connection.response), &transferred, &owner->connection.overlapped)) {
    const DWORD error = owner->connection.Api().last_error();
    if (error != ERROR_IO_PENDING) return owner->connection.Fail(error);
    owner->connection.pending = true;
    const DWORD remaining = owner->connection.Remaining();
    const DWORD waited = owner->connection.Api().wait(owner->connection.event, remaining);
    if (waited != WAIT_OBJECT_0) {
      const DWORD cause = waited == WAIT_TIMEOUT ? ERROR_TIMEOUT : owner->connection.Api().last_error();
      owner->connection.Fail(cause);
      RetireRegistration(owner);
      return false;
    }
    if (!owner->connection.Api().result(owner->connection.pipe, &owner->connection.overlapped, &transferred, FALSE)) {
      const DWORD cause = owner->connection.Api().last_error();
      if (cause == ERROR_OPERATION_ABORTED) owner->connection.pending = false;
      return owner->connection.Fail(cause);
    }
    owner->connection.pending = false;
  }
  if (transferred != sizeof(owner->connection.response)) return owner->connection.Fail(ERROR_INVALID_DATA);
  const DWORD after = owner->connection.Api().wait(owner->process, 0);
  if (after != WAIT_TIMEOUT)
    return owner->connection.Fail(after == WAIT_FAILED ? owner->connection.Api().last_error() : ERROR_PROCESS_ABORTED);
  if (!owner->connection.CheckDeadline()) return false;
  owner->completed = true;
  return RetireRegistration(owner);
}
}  // namespace sugarsubstitute
