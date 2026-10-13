// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/owned_attach.h"

namespace sugarsubstitute {
namespace {
bool LiveProcess(OwnedConnection& connection, HANDLE handle, DWORD expected, uint64_t birth) {
  const DWORD actual = connection.Api().process_id(handle);
  if (!actual) return connection.Fail(connection.Api().last_error());
  uint64_t actual_birth = 0;
  if (!connection.ProcessBirth(handle, &actual_birth)) return false;
  if (actual != expected || actual_birth != birth) return connection.Fail(ERROR_ACCESS_DENIED);
  const DWORD alive = connection.Api().wait(handle, 0);
  return alive == WAIT_TIMEOUT || connection.Fail(
      alive == WAIT_FAILED ? connection.Api().last_error() : ERROR_PROCESS_ABORTED);
}
}  // namespace
bool OwnedAttach::ValidateProcesses() {
  return LiveProcess(connection, creator, creator_pid, creator_birth) &&
         LiveProcess(connection, handler, handler_pid, handler_birth);
}
bool OwnedAttach::Open() {
  connection.pipe = connection.Api().open(endpoint, GENERIC_READ | GENERIC_WRITE, 0,
      nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT |
      SECURITY_IDENTIFICATION, nullptr);
  if (connection.pipe == INVALID_HANDLE_VALUE)
    return connection.Fail(connection.Api().last_error());
  accepted_pipe = connection.pipe;
  ULONG server_pid = 0;
  if (!connection.Api().server_id(connection.pipe, &server_pid))
    return connection.Fail(connection.Api().last_error());
  // Handler Run consumes the inherited creator instance once, then creates its
  // own subsequent instances. Both actual live identities were authenticated.
  if (server_pid != creator_pid && server_pid != handler_pid)
    return connection.Fail(ERROR_ACCESS_DENIED);
  if (!ValidateProcesses()) return false;
  DWORD mode = PIPE_READMODE_MESSAGE;
  if (!connection.Api().mode(connection.pipe, &mode, nullptr, nullptr))
    return connection.Fail(connection.Api().last_error());
  return true;
}
bool OwnedAttach::Issue() {
  if (issued || issuing || connection.pipe == INVALID_HANDLE_VALUE ||
      connection.request.type != crashpad::ClientToServerMessage::kRegister)
    return connection.Fail(ERROR_INVALID_STATE);
  issuing = true;
  // Until the actual call returns, custody conservatively covers possible I/O.
  // Success/IO_PENDING alone sets issued; unwind never grants acceptance proof.
  connection.pending = true;
  DWORD transferred = 0;
  const bool accepted = !!connection.Api().transact(connection.pipe,
      &connection.request, sizeof(connection.request), &connection.response,
      sizeof(connection.response), &transferred, &connection.overlapped);
  const DWORD cause = accepted ? ERROR_SUCCESS : connection.Api().last_error();
  if (!accepted && cause != ERROR_IO_PENDING) {
    connection.pending = false;
    RecordResponse();
    issuing = false;
    return connection.Fail(cause);
  }
  issued = true;
  issuing = false;
  connection.pending = !accepted;
  if (accepted) {
    RecordResponse();
    response_bytes = transferred;
    response_complete = true;
    stage = AttachStage::Validate;
  }
  return true;
}
bool OwnedAttach::Complete() {
  const DWORD signal = connection.Api().wait(connection.event, 0);
  if (signal == WAIT_TIMEOUT) return false;
  if (signal != WAIT_OBJECT_0) return connection.Fail(connection.Api().last_error());
  DWORD transferred = 0;
  if (!connection.Api().result(connection.pipe, &connection.overlapped, &transferred, FALSE)) {
    const DWORD cause = connection.Api().last_error();
    if (cause == ERROR_IO_INCOMPLETE) return false;
    if (cause == ERROR_OPERATION_ABORTED) connection.pending = false;
    if (!connection.pending) RecordResponse();
    return connection.Fail(cause);
  }
  connection.pending = false;
  RecordResponse();
  response_bytes = transferred;
  response_complete = true;
  stage = AttachStage::Validate;
  return true;
}
void OwnedAttach::RecordResponse() {
  if (response_recorded) return;
  response_recorded = true;
  const auto& response = connection.response.registration;
  // The exact response words enter durable unknown slots before validation. An
  // unknown token is never closed as an owned Event, even on rejection/unwind.
  response_events[0] = reinterpret_cast<HANDLE>(static_cast<intptr_t>(response.request_crash_dump_event));
  response_events[1] = reinterpret_cast<HANDLE>(static_cast<intptr_t>(response.request_non_crash_dump_event));
  response_events[2] = reinterpret_cast<HANDLE>(static_cast<intptr_t>(response.non_crash_dump_completed_event));
}
bool OwnedAttach::AdoptResponse() {
  if (!response_complete || response_bytes != sizeof(connection.response))
    return connection.Fail(ERROR_INVALID_DATA);
  for (size_t i = 0; i < 3; ++i) {
    const HANDLE event = response_events[i];
    const uintptr_t value = reinterpret_cast<uintptr_t>(event);
    if (!event || value >= UINTPTR_MAX - 15 || event == creator || event == handler ||
        event == accepted_pipe || event == completion_event)
      return connection.Fail(ERROR_ACCESS_DENIED);
    for (size_t j = 0; j < i; ++j)
      if (event == response_events[j]) return connection.Fail(ERROR_ACCESS_DENIED);
  }
  return ValidateProcesses();
}
bool OwnedAttach::InspectEvents(CleanupLimit* limit) {
  for (size_t i = 0; i < 3; ++i) {
    if (!response_events[i] || response_custody[i] != ResponseEvent::Unknown) continue;
    const HANDLE event = response_events[i];
    const uintptr_t value = reinterpret_cast<uintptr_t>(event);
    if (value >= UINTPTR_MAX - 15 || event == creator || event == handler ||
        event == accepted_pipe || event == completion_event) return false;
    for (size_t j = 0; j < i; ++j) if (event == response_events[j]) return false;
    if (!connection.CleanupAllowed(limit)) return false;
    if (!runtime->inspect_event(this, i)) return false;
    response_custody[i] = ResponseEvent::Owned;
  }
  return true;
}
bool OwnedAttach::CloseEvents() {
  bool closed = true;
  for (size_t i = 0; i < 3; ++i) {
    if (!response_events[i] || response_custody[i] == ResponseEvent::CallbackPinned) continue;
    if (response_custody[i] == ResponseEvent::Unknown) { closed = false; continue; }
    if (!connection.Close(&response_events[i], nullptr, &cleanup_limit)) closed = false;
  }
  return closed;
}
}  // namespace sugarsubstitute
