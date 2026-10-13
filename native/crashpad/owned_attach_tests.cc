// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// CPU-only actual owner tests; only native and Crashpad boundaries are fake.
#include "sugarsubstitute/owned_attach_test_support.h"
namespace {
bool SequentialStages() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  Poll(owner);
  if (owner.stage != AttachStage::Initialize || boundary.opens != 1 || boundary.initializes) return false;
  Poll(owner);
  if (owner.stage != AttachStage::Issue || boundary.initializes != 1 || boundary.issues) return false;
  Poll(owner);
  if (owner.stage != AttachStage::Validate || !owner.issued || boundary.inspected) return false;
  Poll(owner);
  if (owner.stage != AttachStage::Install || boundary.inspected != 3 || boundary.installs) return false;
  Poll(owner);
  if (owner.stage != AttachStage::Publish || boundary.installs != 1 || owner.registered) return false;
  Poll(owner); owner.Retire(Progress(owner));
  return owner.registered && owner.retired && boundary.callbacks == 6 &&
      boundary.closes == 1 && !boundary.borrowed_closes && !boundary.bad_phase;
}
bool ExactCreatorOrHandler() {
  boundary = {}; boundary.server_pid = 71; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner);
  return owner.stage == AttachStage::Initialize && !owner.connection.error;
}
bool ForeignOrigin() {
  boundary = {}; boundary.server_pid = 72; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner);
  if (owner.connection.error != ERROR_ACCESS_DENIED || owner.connection.pipe != Token(1)) return false;
  owner.Retire(Progress(owner)); return owner.retired && boundary.closes == 1 && !boundary.issues;
}
bool ChangedBirth() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  boundary.handler_birth = 203; Poll(owner);
  return owner.connection.error == ERROR_ACCESS_DENIED && !boundary.opens;
}
bool DeadHandler() {
  boundary = {}; boundary.process_wait = WAIT_OBJECT_0; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner);
  return owner.connection.error == ERROR_PROCESS_ABORTED && !boundary.opens;
}
bool CancelledFiniteStage() {
  boundary = {}; boundary.cancel_in_open = true; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner); Poll(owner);
  if (!owner.stopped || boundary.initializes || boundary.issues || owner.connection.pipe != Token(1)) return false;
  owner.Retire(Progress(owner)); return owner.retired && !boundary.borrowed_closes;
}
bool ExpiredAfterFiniteStage() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false; Poll(owner);
  boundary.now = 10000; Poll(owner);
  return owner.expired && owner.connection.error == ERROR_TIMEOUT && !boundary.initializes;
}
bool StepTokenLifetime() {
  boundary = {}; boundary.duplicate_step = true; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner);
  SsCaptureAttachStepV2 old = {2, sizeof(old), reinterpret_cast<uintptr_t>(&owner), 1, 0, owner.nonce};
  return boundary.duplicate_rejected && !owner.Step(old) && boundary.opens == 1;
}
bool ForeignNonce() {
  boundary = {}; boundary.bad_nonce = true; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner);
  return owner.connection.error == ERROR_OPERATION_ABORTED && !boundary.opens;
}
bool PendingRetainsUntilTerminal() {
  boundary = {}; boundary.issue_error = ERROR_IO_PENDING; boundary.event_wait = WAIT_TIMEOUT;
  OwnedAttach owner; if (!Setup(owner)) return false; ToIssue(owner); Poll(owner);
  if (!owner.issued || !owner.connection.pending) return false;
  owner.Retire(Progress(owner));
  if (owner.retired || !owner.connection.pending || boundary.closes || boundary.results) return false;
  boundary.event_wait = WAIT_OBJECT_0; boundary.result_error = ERROR_OPERATION_ABORTED;
  owner.Retire(Progress(owner));
  return owner.retired && !owner.connection.pending && boundary.closes == 4 &&
      !boundary.borrowed_closes && boundary.cleanup_calls == 1;
}
bool CompletionDoesNotIssueNextStage() {
  boundary = {}; boundary.issue_error = ERROR_IO_PENDING; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner); Poll(owner);
  return owner.stage == AttachStage::Validate && !owner.connection.pending &&
      boundary.callbacks == 3 && boundary.results == 1 && !boundary.inspected;
}
bool PartialResponseAdoptedBeforeRejection() {
  boundary = {}; boundary.short_response = true; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner); Poll(owner);
  if (owner.connection.error != ERROR_INVALID_DATA || owner.response_events[2] != Token(12)) return false;
  owner.Retire(Progress(owner));
  return owner.retired && boundary.closes == 4 && !owner.registered;
}
bool UnknownEventCannotClose() {
  boundary = {}; boundary.bad_event = true; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner); Poll(owner);
  owner.Retire(Progress(owner));
  if (owner.retired || boundary.closes != 1 || owner.response_events[0] != Token(10)) return false;
  boundary.bad_event = false; owner.Retire(Progress(owner));
  return owner.retired && boundary.closes == 4;
}
bool FailedCloseRetainsOriginalCause() {
  boundary = {}; boundary.short_response = true; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner); Poll(owner);
  boundary.close_error = ERROR_SHARING_VIOLATION; owner.Retire(Progress(owner));
  if (owner.retired || owner.connection.pipe != Token(1) || owner.connection.error != ERROR_INVALID_DATA ||
      owner.connection.cleanup_error != ERROR_SHARING_VIOLATION) return false;
  boundary.close_error = 0; owner.Retire(Progress(owner));
  return owner.retired && owner.connection.error == ERROR_INVALID_DATA;
}
bool CallbackUnwindRestoresCallToken() {
  boundary = {}; boundary.throw_callback = true; OwnedAttach owner;
  if (!Setup(owner)) return false;
  try { Poll(owner); return false; } catch (int value) { if (value != 19) return false; }
  owner.Retire(Progress(owner));
  return owner.retired && !boundary.opens && boundary.cleanup_calls == 1;
}
bool NativeIssueUnwindRetainsPossibleIo() {
  boundary = {}; boundary.throw_issue = true; boundary.event_wait = WAIT_TIMEOUT; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner);
  try { Poll(owner); return false; } catch (int value) { if (value != 17) return false; }
  if (owner.issued || !owner.issuing || !owner.connection.pending) return false;
  owner.Retire(Progress(owner)); if (owner.retired || boundary.closes) return false;
  boundary.event_wait = WAIT_OBJECT_0; boundary.result_error = ERROR_OPERATION_ABORTED;
  owner.Retire(Progress(owner)); return owner.retired && boundary.closes == 4;
}
bool FactsCacheAndCleanupNeverRenew() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.Retire(Progress(owner)); owner.Retire(Progress(owner));
  SsCaptureAttachResultV2 facts = {}; owner.Facts(&facts);
  return owner.retired && boundary.cleanup_calls == 1 && facts.cleanup_start == 100 &&
      facts.cleanup_grace == 10100 && facts.cleanup_total == 15100 && !facts.registered;
}
bool MixedCausesPreserveFirstAndDomains() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.connection.FailStatus(failed_status);
  owner.connection.Fail(ERROR_ACCESS_DENIED);
  owner.connection.FailErrno(22);
  SsCaptureAttachResultV2 facts = {}; owner.Facts(&facts);
  if (facts.primary_error != static_cast<uint32_t>(failed_status) ||
      facts.primary_error_domain != 2 || facts.cleanup_count != 2 ||
      facts.cleanup_codes[0] != ERROR_ACCESS_DENIED || facts.cleanup_codes[1] != 22 ||
      facts.cleanup_error_domains != (1u | (3u << 2)) || boundary.wrong_error_domain) return false;
  owner.Facts(&facts);
  return facts.primary_error_domain == 2 && !facts.cleanup_count && !facts.cleanup_error_domains;
}
bool ErrnoFirstDoesNotBecomeWin32() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.connection.FailErrno(22); owner.connection.FailStatus(failed_status);
  SsCaptureAttachResultV2 facts = {}; owner.Facts(&facts);
  return facts.primary_error == 22 && facts.primary_error_domain == 3 &&
      facts.cleanup_count == 1 && facts.cleanup_error_domains == 2 &&
      facts.cleanup_codes[0] == static_cast<uint32_t>(failed_status) && !boundary.wrong_error_domain;
}
bool TypedFailureDeadlineReobservationIsPure() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.connection.FailStatus(failed_status);
  const int previous_errors = boundary.set_errors;
  return !owner.connection.CheckDeadline() && !boundary.counters &&
      previous_errors == boundary.set_errors && !owner.connection.cleanup_count &&
      !boundary.wrong_error_domain;
}
bool RuntimeFailureDoesNotFeedRawCauseToFallback() {
  boundary = {}; boundary.initialize_status_failure = true; OwnedAttach owner;
  if (!Setup(owner)) return false; Poll(owner); Poll(owner);
  return owner.connection.error == static_cast<uint32_t>(failed_status) &&
      owner.connection.error_domain == sugarsubstitute::NativeCauseDomain::Ntstatus &&
      !owner.connection.cleanup_count && !boundary.wrong_error_domain;
}
bool SecondaryLedgerIsFixedAndDomainAware() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.connection.FailErrno(22);
  for (int i = 0; i < 10; ++i) {
    if (i % 2) owner.connection.FailStatus(failed_status);
    else owner.connection.Fail(ERROR_SHARING_VIOLATION);
  }
  SsCaptureAttachResultV2 facts = {}; owner.Facts(&facts);
  if (facts.primary_error != 22 || facts.primary_error_domain != 3 ||
      facts.cleanup_count != 8 || facts.omitted_cleanup != 2) return false;
  for (unsigned i = 0; i < 8; ++i) {
    const uint32_t expected_domain = i % 2 ? 2u : 1u;
    if (((facts.cleanup_error_domains >> (i * 2)) & 3) != expected_domain) return false;
  }
  return !(facts.cleanup_error_domains >> 16) && !boundary.wrong_error_domain;
}
bool ZeroNativeCauseIsExplicitFailure() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  owner.connection.Fail(0);
  SsCaptureAttachResultV2 facts = {}; owner.Facts(&facts);
  return facts.primary_error == ERROR_GEN_FAILURE && facts.primary_error_domain == 1;
}
bool LateFirstCloseAdoptsBeforeExpiryAndVetoesRemaining() {
  boundary = {}; boundary.pin_events = false; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner);
  owner.connection.FailStatus(failed_status);
  boundary.close_returns_at = 15100;
  owner.Retire(Progress(owner));
  if (owner.retired || owner.connection.pipe != INVALID_HANDLE_VALUE ||
      boundary.closes != 1 || boundary.inspected || owner.response_events[0] != Token(10) ||
      owner.connection.error != static_cast<uint32_t>(failed_status) ||
      owner.connection.error_domain != sugarsubstitute::NativeCauseDomain::Ntstatus ||
      owner.connection.cleanup_count != 1 || owner.connection.cleanup_codes[0] != ERROR_TIMEOUT)
    return false;
  const int counters = boundary.counters;
  owner.Retire(Progress(owner));
  return !owner.retired && boundary.closes == 1 && boundary.counters == counters &&
      boundary.cleanup_calls == 1 && owner.cleanup.total_counter == 15100;
}
bool LateFailedCloseRetainsSameCellAndBothCauses() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false; Poll(owner);
  owner.connection.FailErrno(22);
  boundary.close_error = ERROR_SHARING_VIOLATION; boundary.close_returns_at = 15100;
  owner.Retire(Progress(owner));
  const int closes = boundary.closes;
  owner.Retire(Progress(owner));
  return !owner.retired && owner.connection.pipe == Token(1) && boundary.closes == closes &&
      owner.connection.error == 22 && owner.connection.error_domain == sugarsubstitute::NativeCauseDomain::Errno &&
      owner.connection.cleanup_count == 2 && owner.connection.cleanup_codes[0] == ERROR_SHARING_VIOLATION &&
      owner.connection.cleanup_codes[1] == ERROR_TIMEOUT && !boundary.wrong_error_domain;
}
bool ExpiredCleanupIssuesNoCloseOrInspection() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false; Poll(owner);
  if (!owner.LatchCleanup(Progress(owner).cleanup)) return false;
  boundary.now = 15100; owner.Retire(Progress(owner));
  return !owner.retired && !boundary.closes && !boundary.inspected &&
      owner.connection.pipe == Token(1) && owner.connection.cleanup_codes[0] == ERROR_TIMEOUT;
}
bool FailedCleanupCounterIssuesNoClose() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false; Poll(owner);
  boundary.counter_failure = true; owner.Retire(Progress(owner));
  const int counters = boundary.counters; owner.Retire(Progress(owner));
  return !owner.retired && !boundary.closes && counters == boundary.counters &&
      owner.connection.cleanup_count == 1 && owner.connection.cleanup_codes[0] == ERROR_ACCESS_DENIED;
}
bool LastAcceptedCloseMayProveOnlyReturnedNativeOutputs() {
  boundary = {}; boundary.short_response = true; OwnedAttach owner;
  if (!Setup(owner)) return false; ToIssue(owner); Poll(owner); Poll(owner);
  boundary.close_returns_at = 15100; boundary.close_advances_at = 4;
  owner.Retire(Progress(owner));
  return owner.retired && boundary.closes == 4 && !boundary.borrowed_closes &&
      owner.connection.pipe == INVALID_HANDLE_VALUE && !owner.response_events[0] &&
      !owner.response_events[1] && !owner.response_events[2] &&
      owner.connection.error == ERROR_INVALID_DATA && !owner.connection.cleanup_count;
}
bool PositiveCleanupRegressionStopsBeforeNextEffect() {
  boundary = {}; OwnedAttach owner; if (!Setup(owner)) return false;
  ToIssue(owner); Poll(owner);
  boundary.now = 200; boundary.close_returns_at = 150;
  owner.Retire(Progress(owner));
  const int counters = boundary.counters;
  owner.Retire(Progress(owner));
  return !owner.retired && owner.connection.pipe == INVALID_HANDLE_VALUE &&
      boundary.closes == 1 && !boundary.inspected && boundary.counters == counters &&
      owner.cleanup_limit.previous_counter == 200 && owner.cleanup_limit.stopped &&
      owner.connection.cleanup_count == 1 && owner.connection.cleanup_codes[0] == ERROR_INVALID_DATA;
}
}  // namespace
int main() {
  const struct { const char* name; bool (*test)(); } cases[] = {
    {"one-finite-stage-per-poll-and-actual-install-before-publication", SequentialStages},
    {"actual-handler-created-instance", ExactCreatorOrHandler}, {"foreign-origin-retains-adopted-pipe", ForeignOrigin},
    {"changed-birth-before-effect", ChangedBirth}, {"dead-handler-before-effect", DeadHandler},
    {"cancel-during-finite-open-vetoes-next-stage", CancelledFiniteStage},
    {"original-deadline-after-open-vetoes-initialize", ExpiredAfterFiniteStage},
    {"one-shot-and-call-borrowed-step", StepTokenLifetime}, {"foreign-nonce-no-effect", ForeignNonce},
    {"pending-message-real-terminal-before-close", PendingRetainsUntilTerminal},
    {"completion-only-poll-no-next-stage", CompletionDoesNotIssueNextStage},
    {"partial-response-retained-before-status", PartialResponseAdoptedBeforeRejection},
    {"unknown-event-retains-until-actual-inspection", UnknownEventCannotClose},
    {"failed-close-original-first-cause", FailedCloseRetainsOriginalCause},
    {"callback-unwind-restores-borrowed-token", CallbackUnwindRestoresCallToken},
    {"issue-unwind-no-false-issued-keeps-possible-io", NativeIssueUnwindRetainsPossibleIo},
    {"cleanup-original-latch-and-cache-only-facts", FactsCacheAndCleanupNeverRenew},
    {"raw-first-and-mixed-secondary-domains", MixedCausesPreserveFirstAndDomains},
    {"errno-first-retains-correct-domain", ErrnoFirstDoesNotBecomeWin32},
    {"typed-deadline-reobservation-pure", TypedFailureDeadlineReobservationIsPure},
    {"runtime-NTSTATUS-no-Win32-fallback-reinterpretation", RuntimeFailureDoesNotFeedRawCauseToFallback},
    {"fixed8-mixed-domain-secondary-ledger", SecondaryLedgerIsFixedAndDomainAware},
    {"zero-native-cause-not-success", ZeroNativeCauseIsExplicitFailure},
    {"late-first-close-adopted-no-second-effect", LateFirstCloseAdoptsBeforeExpiryAndVetoesRemaining},
    {"late-failed-close-same-cell-first-and-secondary-causes", LateFailedCloseRetainsSameCellAndBothCauses},
    {"expiry-no-fresh-close-inspection", ExpiredCleanupIssuesNoCloseOrInspection},
    {"failed-cleanup-counter-no-close-reobservation", FailedCleanupCounterIssuesNoClose},
    {"last-accepted-close-late-return-native-facts-only", LastAcceptedCloseMayProveOnlyReturnedNativeOutputs},
    {"positive-cleanup-counter-regression-no-next-effect", PositiveCleanupRegressionStopsBeforeNextEffect},
  };
  for (const auto& item : cases) {
    if (!item.test()) { std::fprintf(stderr, "FAIL %s\n", item.name); return 1; }
  }
  std::puts("29 CPU-only actual OwnedAttach cases PASS; no real capture/native campaign");
  return 0;
}
