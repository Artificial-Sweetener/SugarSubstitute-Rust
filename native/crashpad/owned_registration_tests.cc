// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// CPU-only tests of the production owner using an entirely fake native boundary.
#include "sugarsubstitute/owned_start.h"
#include <cstdio>

namespace {
struct Boundary {
  int64_t now = 100;
  DWORD error = ERROR_SUCCESS;
  DWORD cancel_error = ERROR_NOT_FOUND;
  DWORD result_error = ERROR_SUCCESS;
  DWORD wait_result = WAIT_OBJECT_0;
  DWORD close_error = ERROR_SUCCESS;
  ULONG server_pid = 61;
  ULONG initial_pid = 61;
  DWORD handler_pid = 71;
  DWORD before_process_wait = WAIT_TIMEOUT;
  DWORD after_process_wait = WAIT_TIMEOUT;
  DWORD open_error = ERROR_SUCCESS;
  bool server_query_failed = false;
  bool late_issue = false;
  bool late_result = false;
  int counter_calls = 0;
  int result_calls = 0;
  int close_calls = 0;
  int transaction_calls = 0;
  int open_calls = 0;
  int process_waits = 0;
} boundary;
BOOL WINAPI Counter(LARGE_INTEGER* value) {
  ++boundary.counter_calls;
  if (boundary.late_issue && boundary.counter_calls == 2) boundary.now = 60100;
  value->QuadPart = boundary.now; return TRUE;
}
BOOL WINAPI Close(HANDLE) {
  ++boundary.close_calls; boundary.error = boundary.close_error;
  return boundary.close_error == ERROR_SUCCESS;
}
DWORD WINAPI LastError() { return boundary.error; }
void WINAPI SetError(DWORD error) { boundary.error = error; }
BOOL WINAPI Cancel(HANDLE, OVERLAPPED*) {
  boundary.error = boundary.cancel_error;
  return boundary.cancel_error == ERROR_SUCCESS;
}
DWORD WINAPI Wait(HANDLE handle, DWORD) {
  if (handle == reinterpret_cast<HANDLE>(4)) {
    boundary.error = ERROR_INVALID_HANDLE;
    return ++boundary.process_waits == 1 ? boundary.before_process_wait : boundary.after_process_wait;
  }
  return boundary.wait_result;
}
BOOL WINAPI Result(HANDLE, OVERLAPPED*, DWORD* transferred, BOOL) {
  ++boundary.result_calls; boundary.error = boundary.result_error;
  *transferred = sizeof(crashpad::ServerToClientMessage);
  return boundary.result_error == ERROR_SUCCESS;
}
HANDLE WINAPI Open(LPCWSTR, DWORD, DWORD, LPSECURITY_ATTRIBUTES, DWORD, DWORD, HANDLE) {
  ++boundary.open_calls;
  boundary.error = boundary.open_error;
  if (boundary.open_error) return INVALID_HANDLE_VALUE;
  return reinterpret_cast<HANDLE>(1);
}
BOOL WINAPI ServerId(HANDLE handle, ULONG* id) {
  boundary.error = ERROR_INVALID_HANDLE;
  *id = handle == reinterpret_cast<HANDLE>(3) ? boundary.initial_pid : boundary.server_pid;
  return !boundary.server_query_failed;
}
DWORD WINAPI ProcessId(HANDLE handle) {
  boundary.error = ERROR_INVALID_HANDLE;
  return handle == reinterpret_cast<HANDLE>(5) ? 61 : boundary.handler_pid;
}
BOOL WINAPI Mode(HANDLE, DWORD*, DWORD*, DWORD*) { return TRUE; }
HANDLE WINAPI Event(LPSECURITY_ATTRIBUTES, BOOL, BOOL, LPCWSTR) { return reinterpret_cast<HANDLE>(2); }
BOOL WINAPI Transact(HANDLE, void*, DWORD, void*, DWORD, DWORD* transferred, OVERLAPPED*) {
  ++boundary.transaction_calls;
  if (boundary.late_result) boundary.now = 60100;
  *transferred = sizeof(crashpad::ServerToClientMessage); return TRUE;
}
const sugarsubstitute::RegistrationApi api = {&Counter, &Close, &LastError,
    &SetError, &Cancel, &Wait, &Result, &Open, &ServerId,
    &ProcessId, &Mode, &Event, &Transact};
sugarsubstitute::OwnedStart Owner(bool pending) {
  sugarsubstitute::OwnedStart owner;
  owner.api = &api; owner.frequency = 1000; owner.deadline_counter = 60100;
  owner.process = reinterpret_cast<HANDLE>(4);
  if (pending) {
    owner.pending = true; owner.pipe = reinterpret_cast<HANDLE>(1);
    owner.event = reinterpret_cast<HANDLE>(2); owner.initial_pipe = reinterpret_cast<HANDLE>(3);
  }
  return owner;
}
sugarsubstitute::OwnedStart PreparedOwner() {
  auto owner = Owner(false);
  owner.process = nullptr;
  owner.initial_pipe = reinterpret_cast<HANDLE>(3);
  owner.bootstrap_process = reinterpret_cast<HANDLE>(5);
  if (!sugarsubstitute::PrepareInheritedRegistration(L"fake", &owner)) return owner;
  owner.process = reinterpret_cast<HANDLE>(4);
  return owner;
}
bool Check(bool condition, const char* name) {
  if (!condition) std::fprintf(stderr, "FAIL %s\n", name);
  return condition;
}
bool UnknownQueryRetains() {
  boundary = {}; boundary.result_error = ERROR_INVALID_HANDLE;
  auto owner = Owner(true);
  if (!Check(!sugarsubstitute::RetireRegistration(&owner) && owner.pending &&
      boundary.close_calls == 0 && owner.cleanup_count == 1 &&
      owner.cleanup_codes[0] == ERROR_INVALID_HANDLE, "unknown-query-retains")) return false;
  boundary.result_error = ERROR_OPERATION_ABORTED;
  return Check(sugarsubstitute::RetireRegistration(&owner) && !owner.pending &&
      boundary.close_calls == 3, "actual-abort-retires");
}
bool IncompleteQueryRetains() {
  boundary = {}; boundary.result_error = ERROR_IO_INCOMPLETE;
  auto owner = Owner(true);
  return Check(!sugarsubstitute::RetireRegistration(&owner) && owner.pending &&
      !owner.cleanup_count && !boundary.close_calls, "incomplete-retains");
}
bool CancelRaceNeedsCompletion() {
  boundary = {}; auto owner = Owner(true);
  return Check(sugarsubstitute::RetireRegistration(&owner) && !owner.pending &&
      boundary.result_calls == 1 && boundary.close_calls == 3, "not-found-queries-real-completion");
}
bool UnsignalledDoesNotQuery() {
  boundary = {}; boundary.wait_result = WAIT_TIMEOUT; auto owner = Owner(true);
  return Check(!sugarsubstitute::RetireRegistration(&owner) && owner.pending &&
      !boundary.result_calls && !boundary.close_calls, "unsignalled-retains");
}
bool CloseErrorsBounded() {
  boundary = {}; boundary.cancel_error = ERROR_ACCESS_DENIED;
  boundary.close_error = ERROR_SHARING_VIOLATION; auto owner = Owner(true);
  for (int i = 0; i < 3; ++i) if (sugarsubstitute::RetireRegistration(&owner)) return false;
  return Check(!owner.pending && owner.pipe != INVALID_HANDLE_VALUE && owner.event &&
      owner.initial_pipe && owner.cleanup_error == ERROR_ACCESS_DENIED &&
      owner.cleanup_count == 8 && owner.omitted_cleanup == 2,
      "failed-closures-retain-first-and-bounded-secondary");
}
bool DeadlineBeforeIssue() {
  boundary = {}; auto owner = PreparedOwner();
  boundary.counter_calls = 0; boundary.late_issue = true;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) && !owner.completed &&
      owner.error == ERROR_TIMEOUT && !boundary.transaction_calls,
      "original-deadline-before-issue");
}
bool DeadlineBeforeSuccess() {
  boundary = {}; auto owner = PreparedOwner(); boundary.late_result = true;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) && !owner.completed &&
      owner.error == ERROR_TIMEOUT && boundary.transaction_calls == 1,
      "late-registration-not-healthy");
}
bool MatchingEndpointRequired() {
  boundary = {}; boundary.server_pid = 72; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.pipe != INVALID_HANDLE_VALUE &&
      owner.error == ERROR_ACCESS_DENIED && !boundary.transaction_calls,
      "foreign-connected-instance-retained-without-authorization");
}
bool InheritedCreatorSucceeds() {
  boundary = {}; auto owner = PreparedOwner();
  return Check(owner.prepared && owner.creator_pid == 61 &&
      sugarsubstitute::ConfirmHandler(&owner) && owner.completed &&
      boundary.open_calls == 1 && boundary.transaction_calls == 1 &&
      boundary.process_waits == 2 && boundary.close_calls == 4 &&
      owner.process == reinterpret_cast<HANDLE>(4), "exact-inherited-endpoint-distinct-live-handler");
}
bool ForeignInitialCreatorRejected() {
  boundary = {}; boundary.initial_pid = 72; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.error == ERROR_ACCESS_DENIED &&
      boundary.open_calls == 0 && !boundary.transaction_calls,
      "foreign-original-server-rejected-before-open");
}
bool MissingPreparationRejected() {
  boundary = {}; auto owner = Owner(false);
  return Check(!sugarsubstitute::ConfirmHandler(&owner) &&
      owner.error == ERROR_INVALID_STATE && !boundary.open_calls &&
      !boundary.transaction_calls, "unprepared-confirmation-cannot-discover-server");
}
bool DuplicatePreparationRejected() {
  boundary = {}; auto owner = PreparedOwner(); owner.process = nullptr;
  return Check(!sugarsubstitute::PrepareInheritedRegistration(L"other", &owner) &&
      owner.prepared && owner.error == ERROR_INVALID_STATE && boundary.open_calls == 1,
      "duplicate-preparation-retains-original-capability");
}
bool MissingOriginalCapabilityRejected() {
  boundary = {}; auto owner = Owner(false); owner.process = nullptr;
  return Check(!sugarsubstitute::PrepareInheritedRegistration(L"fake", &owner) &&
      owner.error == ERROR_INVALID_STATE && !boundary.open_calls,
      "missing-original-capability-rejected");
}
bool CreatorQueryFailureRejected() {
  boundary = {}; boundary.server_query_failed = true; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.error == ERROR_INVALID_HANDLE &&
      !boundary.open_calls, "unknown-creator-rejected");
}
bool BusyInitialInstanceRejected() {
  boundary = {}; boundary.open_error = ERROR_PIPE_BUSY; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.error == ERROR_PIPE_BUSY &&
      boundary.open_calls == 1, "busy-original-instance-does-not-retry-other-instance");
}
bool ExpiredPreparationRejected() {
  boundary = {}; boundary.now = 60100; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.error == ERROR_TIMEOUT &&
      !boundary.open_calls, "expired-preparation-does-not-open");
}
bool LatePreparationRetains() {
  boundary = {}; boundary.late_issue = true; auto owner = PreparedOwner();
  return Check(!owner.prepared && owner.error == ERROR_TIMEOUT &&
      owner.pipe != INVALID_HANDLE_VALUE && !boundary.transaction_calls,
      "late-preparation-retains-endpoint-without-authorizing-launch");
}
bool HandlerQueryFailureRejected() {
  boundary = {}; auto owner = PreparedOwner(); boundary.handler_pid = 0;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) &&
      owner.error == ERROR_INVALID_HANDLE && !boundary.transaction_calls,
      "unknown-handler-process-rejected");
}
bool CreatorCannotBeHandler() {
  boundary = {}; auto owner = PreparedOwner(); boundary.handler_pid = 61;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) &&
      owner.error == ERROR_ACCESS_DENIED && !boundary.transaction_calls,
      "coordinator-handle-cannot-substitute-handler");
}
bool HandlerWaitFailureRejected() {
  boundary = {}; auto owner = PreparedOwner(); boundary.before_process_wait = WAIT_FAILED;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) &&
      owner.error == ERROR_INVALID_HANDLE && !boundary.transaction_calls,
      "unknown-handler-liveness-rejected");
}
bool ExitedHandlerRejected() {
  boundary = {}; auto owner = PreparedOwner(); boundary.before_process_wait = WAIT_OBJECT_0;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) &&
      owner.error == ERROR_PROCESS_ABORTED && !boundary.transaction_calls,
      "already-exited-handler-does-not-issue");
}
bool LateHandlerExitRejected() {
  boundary = {}; auto owner = PreparedOwner(); boundary.after_process_wait = WAIT_OBJECT_0;
  return Check(!sugarsubstitute::ConfirmHandler(&owner) && !owner.completed &&
      owner.error == ERROR_PROCESS_ABORTED && boundary.transaction_calls == 1 &&
      !boundary.close_calls, "handler-exits-before-response-publication-retains-custody");
}
}  // namespace
int main() {
  const bool passed = UnknownQueryRetains() && IncompleteQueryRetains() &&
      CancelRaceNeedsCompletion() && UnsignalledDoesNotQuery() &&
      CloseErrorsBounded() && DeadlineBeforeIssue() && DeadlineBeforeSuccess() &&
      MatchingEndpointRequired() && InheritedCreatorSucceeds() &&
      ForeignInitialCreatorRejected() && MissingPreparationRejected() &&
      DuplicatePreparationRejected() && MissingOriginalCapabilityRejected() &&
      CreatorQueryFailureRejected() && BusyInitialInstanceRejected() &&
      ExpiredPreparationRejected() && LatePreparationRetains() &&
      HandlerQueryFailureRejected() && CreatorCannotBeHandler() &&
      HandlerWaitFailureRejected() && ExitedHandlerRejected() && LateHandlerExitRejected();
  if (passed) std::puts("22 CPU-only production-registration boundary cases PASS; no native calls");
  return passed ? 0 : 1;
}
