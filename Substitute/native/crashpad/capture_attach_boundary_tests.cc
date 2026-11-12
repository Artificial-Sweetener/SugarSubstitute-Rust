// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Proposed actual ABI owner; external native/runtime fakes are copied by exact
// source extraction from the already-qualified OwnedAttach test boundary.
#include "sugarsubstitute/capture_attach_boundary.h"
#include "sugarsubstitute/owned_attach_test_support.h"

namespace {
struct AdmissionBoundary {
  int frequencies = 0, counters = 0;
  int64_t frequency = 1000, now = 100;
  bool frequency_ok = true, counter_ok = true;
  sugarsubstitute::CaptureAttachBoundary* abi = nullptr;
  bool duplicate = false, bad_nonce = false, rejected = false, ledger_unchanged = false;
  SsCaptureAttachResultV2 nested = {};
} admission;
BOOL WINAPI Frequency(LARGE_INTEGER* output) {
  ++admission.frequencies; output->QuadPart = admission.frequency;
  return admission.frequency_ok;
}
BOOL WINAPI AdmissionCounter(LARGE_INTEGER* output) {
  ++admission.counters; output->QuadPart = admission.now;
  return admission.counter_ok;
}
const sugarsubstitute::CaptureBoundaryApi admission_api = {&Frequency, &AdmissionCounter, &LastError};
uint32_t __cdecl BoundaryClaim(uintptr_t, uintptr_t owner, uint32_t stage, uint64_t nonce) {
  ++boundary.callbacks;
  if (boundary.decision != 1) return boundary.decision;
  SsCaptureAttachStepV2 step = {2, sizeof(step), owner, stage, 0,
                              admission.bad_nonce ? nonce + 1 : nonce};
  boundary.in_callback = true;
  admission.abi->Step(&step, &admission.nested);
  const bool accepted = admission.nested.status != 2 && !admission.nested.primary_error;
  if (admission.duplicate) {
    admission.abi->Step(&step, &admission.nested);
    admission.rejected = admission.nested.status == 2 &&
                         admission.nested.primary_error == ERROR_INVALID_STATE;
    admission.ledger_unchanged = !reinterpret_cast<OwnedAttach*>(owner)->connection.error;
  }
  boundary.in_callback = false;
  return accepted ? 1 : 4;
}
struct Fixture {
  OwnedAttach owner;
  std::atomic<bool> attempted{false};
  sugarsubstitute::CaptureAttachBoundary abi;
  SsCaptureAttachInputV2 input;
  SsCaptureAttachResultV2 output = {};
  Fixture() : abi(owner, attempted, runtime, &admission_api), input(AttachInput()) {
    boundary = {}; admission = {};
    admission.abi = &abi;
    owner.connection.boundary = &api;
    input.issue.request = &BoundaryClaim;
  }
  void Start() { abi.Start(&input, &output); }
  void Poll() {
    auto progress = Progress(owner); progress.issue.request = &BoundaryClaim;
    abi.Poll(&progress, &output);
  }
  void Close() {
    auto progress = Progress(owner); progress.issue.request = &BoundaryClaim;
    abi.Retire(&progress, &output);
  }
};
bool MalformedBeforeObservation() {
  Fixture f; f.input.byte_size = 760; f.Start();
  return f.output.status == 2 && f.output.primary_error == ERROR_INVALID_PARAMETER &&
         !f.attempted && !f.owner.started && !admission.frequencies && !boundary.opens;
}
bool NativeFrequencyFailure() {
  Fixture f; admission.frequency_ok = false; boundary.error = ERROR_ACCESS_DENIED; f.Start();
  return f.output.primary_error == ERROR_ACCESS_DENIED && f.output.primary_error_domain == 1 &&
         !f.attempted && !f.owner.started && admission.frequencies == 1 && !admission.counters;
}
bool FrequencyMismatch() {
  Fixture f; admission.frequency = 1001; f.Start();
  return f.output.primary_error == ERROR_INVALID_DATA && !f.attempted && !admission.counters;
}
bool CounterFailureZeroCode() {
  Fixture f; admission.counter_ok = false; f.Start();
  return f.output.primary_error == ERROR_GEN_FAILURE && !f.attempted && !boundary.opens;
}
bool EqualityExpired() {
  Fixture f; admission.now = f.input.attach_counter; f.Start();
  return f.output.primary_error == ERROR_TIMEOUT && !f.attempted && !f.owner.started;
}
bool ParentAlreadyClaimed() {
  Fixture f; f.attempted = true; f.Start();
  return f.output.primary_error == ERROR_ALREADY_INITIALIZED && !f.owner.started && !boundary.opens;
}
bool StartOneActualStage() {
  Fixture f; f.Start();
  const bool observed = f.attempted && f.owner.started && f.owner.stage == AttachStage::Initialize &&
      f.output.attach == reinterpret_cast<uintptr_t>(&f.owner) && f.output.status == 5 &&
      boundary.opens == 1 && !boundary.initializes && !boundary.issues && !boundary.bad_phase;
  f.Close(); return observed && f.owner.retired && !boundary.borrowed_closes;
}
bool PartialOpenFailureAdopted() {
  Fixture f; boundary.server_pid = 99; f.Start();
  const bool observed = f.output.status == 2 && f.output.primary_error == ERROR_ACCESS_DENIED &&
      f.output.attach == reinterpret_cast<uintptr_t>(&f.owner) && f.owner.connection.pipe == Token(1);
  f.Close(); return observed && f.owner.retired && boundary.closes == 1;
}
bool ForeignProgressNeverDereferenced() {
  Fixture f; f.Start(); auto progress = Progress(f.owner); progress.attach = 12345;
  f.abi.Poll(&progress, &f.output);
  const bool observed = f.output.status == 2 && !f.output.attach &&
      boundary.opens == 1 && !boundary.initializes && !f.owner.connection.error;
  f.Close(); return observed && f.owner.retired;
}
bool MalformedProgressKeepsPriorOwner() {
  Fixture f; f.Start(); auto progress = Progress(f.owner); progress.byte_size = 40;
  f.abi.Retire(&progress, &f.output);
  const bool observed = f.output.status == 2 && !f.owner.stopped && !boundary.closes;
  f.Close(); return observed && f.owner.retired;
}
bool StaleStepReportsWithoutNativeIssue() {
  Fixture f; admission.bad_nonce = true; f.Start();
  const bool observed = admission.nested.status == 2 &&
      admission.nested.primary_error == ERROR_INVALID_STATE &&
      admission.nested.attach == reinterpret_cast<uintptr_t>(&f.owner) &&
      f.owner.connection.error == ERROR_OPERATION_ABORTED && !boundary.opens && !boundary.issues;
  f.Close(); return observed && f.owner.retired;
}
bool DuplicateStepKeepsActualIssuedCustody() {
  Fixture f; f.Start(); f.Poll(); admission.duplicate = true;
  boundary.issue_error = ERROR_IO_PENDING; f.Poll();
  const bool observed = admission.rejected && admission.ledger_unchanged &&
      admission.nested.attach == reinterpret_cast<uintptr_t>(&f.owner) &&
      admission.nested.pending_io == 1 && f.owner.issued && f.owner.connection.pending &&
      boundary.issues == 1 && !boundary.bad_phase;
  boundary.result_error = ERROR_OPERATION_ABORTED;
  f.Close(); return observed && f.owner.retired && !boundary.borrowed_closes;
}
bool OutOfCallStepRejectsWithoutEffects() {
  Fixture f; f.Start();
  SsCaptureAttachStepV2 step = {2, sizeof(step), reinterpret_cast<uintptr_t>(&f.owner),
                             static_cast<uint32_t>(f.owner.stage), 0, f.owner.nonce};
  f.abi.Step(&step, &f.output);
  const bool observed = f.output.status == 2 && f.output.primary_error == ERROR_INVALID_STATE &&
      !f.owner.connection.error && !boundary.initializes && !boundary.issues;
  f.Close(); return observed && f.owner.retired;
}
bool NullOutputHasNoAdmission() {
  Fixture f; f.abi.Start(&f.input, nullptr);
  return !f.attempted && !admission.frequencies && !boundary.opens;
}
}
int main() {
  struct Case { const char* name; bool (*run)(); } cases[] = {
    {"malformed-before-native-observation", &MalformedBeforeObservation},
    {"actual-native-frequency-failure", &NativeFrequencyFailure},
    {"exact-frequency-mismatch", &FrequencyMismatch},
    {"failed-counter-zero-code-normalized", &CounterFailureZeroCode},
    {"original-deadline-equality-expired", &EqualityExpired},
    {"parent-role-already-claimed", &ParentAlreadyClaimed},
    {"start-one-actual-finite-stage", &StartOneActualStage},
    {"partial-open-output-before-failure", &PartialOpenFailureAdopted},
    {"foreign-progress-no-token-dereference", &ForeignProgressNeverDereferenced},
    {"malformed-retire-preserves-owner", &MalformedProgressKeepsPriorOwner},
    {"stale-step-local-rejection-no-effects", &StaleStepReportsWithoutNativeIssue},
    {"duplicate-step-retains-original-issued-custody", &DuplicateStepKeepsActualIssuedCustody},
    {"step-outside-original-callback-rejected", &OutOfCallStepRejectsWithoutEffects},
    {"null-output-no-admission", &NullOutputHasNoAdmission},
  };
  for (const auto& test : cases)
    if (!test.run()) { std::fprintf(stderr, "FAIL: %s\n", test.name); return 1; }
  std::puts("14 proposed actual ABI-owner CPU boundary cases PASS; no native campaign");
}
