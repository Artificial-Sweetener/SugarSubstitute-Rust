// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_OWNED_ATTACH_TEST_SUPPORT_H_
#define SUGARSUBSTITUTE_OWNED_ATTACH_TEST_SUPPORT_H_
// CPU-only actual owner tests; only native and Crashpad boundaries are fake.
#include "sugarsubstitute/owned_attach.h"
#include <cstdio>
#include <cstring>

namespace {
using sugarsubstitute::OwnedAttach;
using sugarsubstitute::AttachStage;
HANDLE Token(uintptr_t value) { return reinterpret_cast<HANDLE>(value); }
struct Boundary {
  int64_t now = 100;
  DWORD error = 0, close_error = 0, issue_error = 0, result_error = 0;
  int64_t close_returns_at = 0;
  int close_advances_at = 1;
  bool counter_failure = false;
  DWORD server_pid = 61, process_wait = WAIT_TIMEOUT, event_wait = WAIT_OBJECT_0;
  uint64_t creator_birth = 101, handler_birth = 202;
  uint32_t decision = 1;
  int opens = 0, issues = 0, results = 0, closes = 0, borrowed_closes = 0;
  int callbacks = 0, initializes = 0, inspected = 0, installs = 0, cleanup_calls = 0;
  int counters = 0, set_errors = 0;
  bool in_callback = false, bad_phase = false, cancel_in_open = false;
  bool duplicate_step = false, duplicate_rejected = false, bad_nonce = false;
  bool throw_callback = false, throw_issue = false, short_response = false;
  bool bad_event = false, pin_events = true;
  bool initialize_status_failure = false, wrong_error_domain = false;
} boundary;
constexpr int32_t failed_status = static_cast<int32_t>(0xc0000008u);
BOOL WINAPI Counter(LARGE_INTEGER* value) {
  ++boundary.counters; value->QuadPart = boundary.now;
  if (boundary.counter_failure) { boundary.error = ERROR_ACCESS_DENIED; return FALSE; }
  return TRUE;
}
BOOL WINAPI Close(HANDLE value) {
  ++boundary.closes;
  if (value == Token(2) || value == Token(4) || value == Token(5)) ++boundary.borrowed_closes;
  boundary.error = boundary.close_error;
  if (boundary.close_returns_at && boundary.closes == boundary.close_advances_at)
    boundary.now = boundary.close_returns_at;
  return boundary.close_error == 0;
}
DWORD WINAPI LastError() { return boundary.error; }
void WINAPI SetError(DWORD value) {
  ++boundary.set_errors;
  if (value == static_cast<DWORD>(failed_status) || value == 22)
    boundary.wrong_error_domain = true;
  boundary.error = value;
}
BOOL WINAPI Cancel(HANDLE, OVERLAPPED*) { boundary.error = ERROR_NOT_FOUND; return FALSE; }
DWORD WINAPI Wait(HANDLE handle, DWORD duration) {
  if (duration != 0) boundary.bad_phase = true;
  return handle == Token(2) ? boundary.event_wait : boundary.process_wait;
}
BOOL WINAPI Result(HANDLE, OVERLAPPED*, DWORD* bytes, BOOL blocking) {
  ++boundary.results;
  if (blocking) boundary.bad_phase = true;
  *bytes = sizeof(crashpad::ServerToClientMessage);
  boundary.error = boundary.result_error;
  return boundary.result_error == 0;
}
HANDLE WINAPI Open(LPCWSTR, DWORD, DWORD, LPSECURITY_ATTRIBUTES, DWORD, DWORD, HANDLE) {
  ++boundary.opens;
  if (boundary.in_callback) boundary.bad_phase = true;
  if (boundary.cancel_in_open) boundary.decision = 2;
  return Token(1);
}
BOOL WINAPI ServerId(HANDLE, ULONG* id) { *id = boundary.server_pid; return TRUE; }
DWORD WINAPI ProcessId(HANDLE value) { return value == Token(5) ? 61 : 71; }
BOOL WINAPI Times(HANDLE value, FILETIME* creation, FILETIME*, FILETIME*, FILETIME*) {
  const uint64_t birth = value == Token(5) ? boundary.creator_birth : boundary.handler_birth;
  creation->dwLowDateTime = static_cast<DWORD>(birth);
  creation->dwHighDateTime = static_cast<DWORD>(birth >> 32); return TRUE;
}
BOOL WINAPI Mode(HANDLE, DWORD*, DWORD*, DWORD*) { return TRUE; }
HANDLE WINAPI Event(LPSECURITY_ATTRIBUTES, BOOL, BOOL, LPCWSTR) { return Token(2); }
BOOL WINAPI Transact(HANDLE, void*, DWORD, void* output, DWORD, DWORD* bytes, OVERLAPPED*) {
  ++boundary.issues;
  if (!boundary.in_callback) boundary.bad_phase = true;
  auto* response = static_cast<crashpad::ServerToClientMessage*>(output);
  response->registration.request_crash_dump_event = 10;
  response->registration.request_non_crash_dump_event = 11;
  response->registration.non_crash_dump_completed_event = 12;
  *bytes = boundary.short_response ? 4 : sizeof(*response);
  boundary.error = boundary.issue_error;
  if (boundary.throw_issue) throw 17;
  return boundary.issue_error == 0;
}
const sugarsubstitute::ConnectionApi api = {&Counter, &Close, &LastError, &SetError,
    &Cancel, &Wait, &Result, &Open, &ServerId, &ProcessId, &Mode, &Event, &Transact, &Times};
bool Initialize(OwnedAttach* owner) {
  ++boundary.initializes;
  if (boundary.in_callback) boundary.bad_phase = true;
  return !boundary.initialize_status_failure || owner->connection.FailStatus(failed_status);
}
bool Inspect(OwnedAttach*, size_t) {
  ++boundary.inspected;
  if (boundary.in_callback) boundary.bad_phase = true;
  return !boundary.bad_event;
}
bool Validate(OwnedAttach* owner) {
  if (boundary.in_callback) boundary.bad_phase = true;
  if (boundary.pin_events) {
    for (auto& custody : owner->response_custody)
      custody = sugarsubstitute::ResponseEvent::CallbackPinned;
    owner->globals_committed = true;
  }
  return boundary.pin_events;
}
bool Install(OwnedAttach*) {
  ++boundary.installs;
  if (boundary.in_callback) boundary.bad_phase = true;
  return true;
}
const sugarsubstitute::AttachRuntime runtime = {&Initialize, &Inspect, &Validate, &Install};
bool __cdecl Cleanup(uintptr_t, SsCaptureCleanupBudgetV1* output) {
  ++boundary.cleanup_calls;
  *output = {1, sizeof(*output), 1, 0, 100, 10100, 15100, 1000}; return true;
}
uint32_t __cdecl Claim(uintptr_t, uintptr_t attach, uint32_t stage, uint64_t nonce) {
  ++boundary.callbacks;
  if (boundary.throw_callback) throw 19;
  if (boundary.decision != 1) return boundary.decision;
  auto* owner = reinterpret_cast<OwnedAttach*>(attach);
  SsCaptureAttachStepV2 step = {2, sizeof(step), attach, stage, 0,
      boundary.bad_nonce ? nonce + 1 : nonce};
  boundary.in_callback = true;
  try {
    const bool accepted = owner->Step(step);
    if (boundary.duplicate_step) boundary.duplicate_rejected = !owner->Step(step);
    boundary.in_callback = false;
    return accepted ? 1 : 4;
  } catch (...) { boundary.in_callback = false; throw; }
}
SsCaptureAttachProgressV2 Progress(OwnedAttach& owner) {
  return {2, sizeof(SsCaptureAttachProgressV2), reinterpret_cast<uintptr_t>(&owner),
      {1, &Cleanup}, {1, &Claim}};
}
SsCaptureAttachInputV2 AttachInput() {
  SsCaptureAttachInputV2 input = {};
  input.abi_version = 2; input.byte_size = sizeof(input);
  input.subject = input.provenance = 1; input.coordinator_epoch[0] = 1;
  input.instance = input.generation = 1;
  input.creator_pid = 61; input.handler_pid = 71;
  input.creator_birth = 101; input.handler_birth = 202;
  input.creator_process = 5; input.handler_process = 4;
  input.attach_counter = 10000; input.counter_frequency = 1000;
  input.build[0] = 'a'; input.build_length = 1;
  const uint16_t name[] = {'\\', '\\', '.', '\\', 'p', 'i', 'p', 'e', '\\', 'x'};
  std::memcpy(input.endpoint, name, sizeof(name)); input.endpoint_length = 10;
  input.cleanup = {1, &Cleanup}; input.issue = {1, &Claim}; input.borrowed_completion_event = 2;
  return input;
}
bool Setup(OwnedAttach& owner) {
  owner.connection.boundary = &api; owner.runtime = &runtime;
  return owner.Begin(AttachInput());
}
void Poll(OwnedAttach& owner) { owner.Poll(Progress(owner)); }
void ToIssue(OwnedAttach& owner) { Poll(owner); Poll(owner); }

} // namespace
#endif
