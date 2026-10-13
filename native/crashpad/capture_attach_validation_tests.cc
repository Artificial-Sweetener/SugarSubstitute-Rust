// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_attach_validation.h"
#include "sugarsubstitute/capture_bridge.h"
#include <stdio.h>
#include <string.h>
#include <limits>

namespace {
bool __cdecl Cleanup(uintptr_t, SsCaptureCleanupBudgetV1*) { return false; }
uint32_t __cdecl Issue(uintptr_t, uintptr_t, uint32_t, uint64_t) { return 4; }
SsCaptureAttachInputV2 Input() {
  SsCaptureAttachInputV2 input = {};
  input.abi_version = 2;
  input.byte_size = sizeof(input);
  input.subject = input.provenance = 1;
  input.coordinator_epoch[0] = 1;
  input.instance = input.generation = 1;
  input.creator_pid = 10;
  input.handler_pid = 11;
  input.creator_birth = 100;
  input.handler_birth = 101;
  input.creator_process = 200;
  input.handler_process = 201;
  input.attach_counter = 10000;
  input.counter_frequency = 1000;
  input.build[0] = 'a';
  input.build_length = 1;
  const uint16_t endpoint[] = {'\\', '\\', '.', '\\', 'p', 'i', 'p', 'e', '\\', 'x'};
  memcpy(input.endpoint, endpoint, sizeof(endpoint));
  input.endpoint_length = 10;
  input.cleanup = {1, &Cleanup};
  input.issue = {1, &Issue};
  input.borrowed_completion_event = 202;
  return input;
}
bool Reject(const SsCaptureAttachInputV2& input) {
  return !sugarsubstitute::ValidAttachInput(input);
}
bool ExactLayouts() {
  return sizeof(SsCaptureInput) == 96 && sizeof(SsCaptureResult) == 88 &&
         sizeof(SsCaptureEndpointExpectationV1) == 32 && sizeof(SsCaptureEndpointV1) == 560 &&
         sizeof(SsCaptureAttachInputV2) == 776 && sizeof(SsCaptureAttachResultV2) == 120 &&
         sizeof(SsCaptureAttachProgressV2) == 48 && sizeof(SsCaptureAttachStepV2) == 32 &&
         sizeof(SsCaptureCleanupBudgetV1) == 48 && sizeof(SsCaptureSourceIdentityV1) == 40;
}
bool ExactInput() { return sugarsubstitute::ValidAttachInput(Input()); }
bool Historical760Rejected() { auto input = Input(); input.byte_size = 760; return Reject(input); }
bool WrongVersionRejected() { auto input = Input(); input.abi_version = 1; return Reject(input); }
bool WrongPhysicalSizeRejected() {
  auto input = Input(); input.byte_size = 775;
  if (!Reject(input)) return false;
  input.byte_size = 777; return Reject(input);
}
bool ForeignSubjectProvenanceRejected() {
  auto input = Input(); input.subject = 2;
  if (!Reject(input)) return false;
  input.subject = 1; input.provenance = 2; return Reject(input);
}
bool EmptyRunEpochRejected() {
  auto input = Input(); input.instance = 0;
  if (!Reject(input)) return false;
  input = Input(); input.generation = 0;
  if (!Reject(input)) return false;
  input = Input(); input.coordinator_epoch[0] = 0; return Reject(input);
}
bool AliasIdentityRejected() {
  auto input = Input(); input.handler_pid = input.creator_pid;
  if (!Reject(input)) return false;
  input = Input(); input.handler_process = input.creator_process; return Reject(input);
}
bool MissingBirthRejected() {
  auto input = Input(); input.creator_birth = 0;
  if (!Reject(input)) return false;
  input = Input(); input.handler_birth = 0; return Reject(input);
}
bool MissingPseudoAndAliasedEventRejected() {
  auto input = Input(); input.borrowed_completion_event = 0;
  if (!Reject(input)) return false;
  input.borrowed_completion_event = std::numeric_limits<uintptr_t>::max();
  if (!Reject(input)) return false;
  input.borrowed_completion_event = input.handler_process; return Reject(input);
}
bool CounterAndReservedRejected() {
  auto input = Input(); input.attach_counter = 0;
  if (!Reject(input)) return false;
  input = Input(); input.counter_frequency = 0;
  if (!Reject(input)) return false;
  input = Input(); input.reserved_deadline0 = 1;
  if (!Reject(input)) return false;
  input = Input(); input.reserved_deadline1 = 1; return Reject(input);
}
bool InvalidBuildAndTailRejected() {
  auto input = Input(); input.build[0] = '/';
  if (!Reject(input)) return false;
  input = Input(); input.build[1] = 'a';
  if (!Reject(input)) return false;
  input = Input(); input.build_length = 65; return Reject(input);
}
bool InvalidEndpointAndTailRejected() {
  auto input = Input(); input.endpoint[0] = 'x';
  if (!Reject(input)) return false;
  input = Input(); input.endpoint[9] = 0;
  if (!Reject(input)) return false;
  input = Input(); input.endpoint[10] = 'x';
  if (!Reject(input)) return false;
  input = Input(); input.endpoint_length = 257; return Reject(input);
}
bool ExactMaximumStringsAccepted() {
  auto input = Input();
  for (auto& c : input.build) c = 'a';
  input.build_length = 64;
  for (size_t i = 9; i < 256; ++i) input.endpoint[i] = 'x';
  input.endpoint_length = 256;
  return sugarsubstitute::ValidAttachInput(input);
}
bool MissingBorrowedCallbacksRejected() {
  auto input = Input(); input.cleanup.request = nullptr;
  if (!Reject(input)) return false;
  input = Input(); input.issue.borrowed_context = 0; return Reject(input);
}
bool ProgressShapeRejected() {
  const auto input = Input();
  SsCaptureAttachProgressV2 progress = {2, 48, 300, input.cleanup, input.issue};
  if (!sugarsubstitute::ValidAttachProgress(progress)) return false;
  progress.byte_size = 32; return !sugarsubstitute::ValidAttachProgress(progress);
}
bool StepShapeNonceAndStageRejected() {
  SsCaptureAttachStepV2 step = {2, 32, 300, 1, 0, 1};
  if (!sugarsubstitute::ValidAttachStep(step)) return false;
  step.nonce = 0;
  if (sugarsubstitute::ValidAttachStep(step)) return false;
  step.nonce = 1; step.stage = 7;
  if (sugarsubstitute::ValidAttachStep(step)) return false;
  step.stage = 6; step.reserved = 1; return !sugarsubstitute::ValidAttachStep(step);
}
}  // namespace

int main() {
  const struct { const char* name; bool (*run)(); } cases[] = {
    {"ExactLayouts", ExactLayouts}, {"ExactInput", ExactInput},
    {"Historical760Rejected", Historical760Rejected}, {"WrongVersionRejected", WrongVersionRejected},
    {"WrongPhysicalSizeRejected", WrongPhysicalSizeRejected},
    {"ForeignSubjectProvenanceRejected", ForeignSubjectProvenanceRejected},
    {"EmptyRunEpochRejected", EmptyRunEpochRejected}, {"AliasIdentityRejected", AliasIdentityRejected},
    {"MissingBirthRejected", MissingBirthRejected},
    {"MissingPseudoAndAliasedEventRejected", MissingPseudoAndAliasedEventRejected},
    {"CounterAndReservedRejected", CounterAndReservedRejected},
    {"InvalidBuildAndTailRejected", InvalidBuildAndTailRejected},
    {"InvalidEndpointAndTailRejected", InvalidEndpointAndTailRejected},
    {"ExactMaximumStringsAccepted", ExactMaximumStringsAccepted},
    {"MissingBorrowedCallbacksRejected", MissingBorrowedCallbacksRejected},
    {"ProgressShapeRejected", ProgressShapeRejected},
    {"StepShapeNonceAndStageRejected", StepShapeNonceAndStageRejected},
  };
  for (const auto& test : cases) {
    if (!test.run()) { fprintf(stderr, "FAIL %s\n", test.name); return 1; }
    printf("PASS %s\n", test.name);
  }
  printf("PASS 17 inert ABI/layout boundary cases; no native identity authority\n");
  return 0;
}
