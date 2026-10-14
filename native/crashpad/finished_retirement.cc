// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <cstring>
#include <new>

namespace sugarsubstitute::finished {
bool Invocation::Absent(Slot parent, const wchar_t* leaf) noexcept {
  if (state.handles[Candidate] || !Allowed()) return false;
  const auto error = state.api.Open(state.handles[parent], leaf, OpenMode::Read, state.handles[Candidate], *this);
  const bool current = Allowed();
  if (state.handles[Candidate]) {
    // Even an error-returned output is retained. Presence never proves absence.
    if (error) state.Fail(error); else state.Fail(ERROR_FILE_EXISTS);
    return false;
  }
  if (!current) return false;
  if (error == ERROR_FILE_NOT_FOUND) return true;
  return state.Fail(error ? error : ERROR_INVALID_HANDLE);
}
bool Invocation::Retire() noexcept {
  if (!Allowed()) return false;
  if (!RetireDigest()) return false;
  if (!UnlockMetadata()) return false;
  // Close dependants before the protected ancestry. Stop on uncertainty.
  constexpr std::array<Slot, SlotCount> order{Candidate, Source, OriginalStage,
      CandidateStage, ScanDirectory, Metadata, Reports, Metrics, Database};
  for (auto slot : order) if (!Close(slot)) return false;
  return true;
}
namespace {
bool Call(Invocation& invocation, const SsFinishedCallV1& call, SsFinishedPhaseV1 phase) noexcept {
  auto& owner = invocation.state;
  if (call.abi_version != 1 || call.phase != phase || !ValidDeadline(call.original) ||
      call.original.counter_frequency != owner.operation.counter_frequency) return owner.Fail(ERROR_INVALID_PARAMETER);
  owner.in_cleanup = phase == Cleanup;
  if (phase == Operation) {
    if (owner.primary || call.original.counter_deadline > owner.operation.counter_deadline) return owner.Fail(ERROR_INVALID_STATE);
    owner.operation.counter_deadline = call.original.counter_deadline;
  } else if (!owner.cleanup_latched) {
    owner.cleanup = call.original; owner.cleanup_latched = true;
  } else if (call.original.counter_deadline > owner.cleanup.counter_deadline) return owner.Fail(ERROR_INVALID_STATE);
  return invocation.Allowed();
}
void Invalid(SsFinishedResultV1* output) noexcept {
  *output = {}; output->abi_version = 1; output->status = Failed; output->primary_error = ERROR_INVALID_PARAMETER;
  output->primary_origin=FinishedCauseNative;
}
}  // namespace
}  // namespace sugarsubstitute::finished

using namespace sugarsubstitute::finished;
void ss_finished_open_v1(const SsFinishedOpenInputV1* input, SsFinishedResultV1* output) {
  if (!output) return;
  Invalid(output);
  if (!input || input->abi_version != 1 || input->reserved || !input->borrowed_database ||
      !input->borrowed_metrics || !ValidSubject(input->subject) ||
      !ValidBuild(input->build_length, input->build) || !ValidDeadline(input->original_operation) ||
      input->has_uuid > 1 || (input->has_uuid == 0 &&
          std::any_of(std::begin(input->optional_uuid), std::end(input->optional_uuid), [](auto b) { return !!b; }))) return;
  if (input->has_uuid && std::all_of(std::begin(input->optional_uuid), std::end(input->optional_uuid), [](auto b) { return !b; })) return;
  auto owner=CreateLease(WindowsFiles(),input->current,input->original_operation,*output,NativeLeaseAllocation);
  if(!owner)return;
  auto& expected = owner->expected;
  expected.borrowed_database = input->borrowed_database; expected.borrowed_metrics = input->borrowed_metrics;
  memcpy(expected.expected_database_identity,input->expected_database_identity,24);
  memcpy(expected.expected_metrics_identity,input->expected_metrics_identity,24);
  expected.subject = input->subject; expected.build_length = input->build_length;
  memcpy(expected.build,input->build,64);memcpy(expected.optional_uuid,input->optional_uuid,16);expected.has_uuid=input->has_uuid;
  owner->Result(*output);  // publish the real token before the first acquisition.
  Invocation invocation(*owner,input->current);
  Query(invocation); owner->Result(*output);
}
void ss_finished_read_v1(const SsFinishedReadInputV1* input, SsFinishedResultV1* output) {
  if (!output) return;
  Invalid(output);
  if (!input || !input->call.lease) return;
  auto& owner = *reinterpret_cast<Lease*>(input->call.lease);
  if (!TryAcquireSRWLockExclusive(&owner.gate)) { BusyResult(*output, input->call.lease); return; }
  Invocation invocation(owner,input->call.current);
  owner.read_bytes = 0;
  if (Call(invocation, input->call, Operation) && owner.facts_valid && !owner.disposed &&
      !input->reserved && input->output && input->capacity && input->capacity <= 65536 &&
      input->offset <= owner.facts.source_bytes && input->capacity <= owner.facts.source_bytes - input->offset) {
    owner.read_slot = Source; owner.read_size = owner.facts.source_bytes;
    if (invocation.Read(input->offset, std::span(input->output, input->capacity)).complete) {
      owner.read_bytes = input->capacity; owner.status = Finished;
    }
  } else if (!owner.primary) owner.Fail(ERROR_INVALID_PARAMETER);
  owner.Result(*output); ReleaseSRWLockExclusive(&owner.gate);
}
void ss_finished_retire_v1(const SsFinishedCallV1* input, SsFinishedResultV1* output) {
  if (!output) return;
  Invalid(output);
  if (!input || !input->lease) return;
  auto owner = reinterpret_cast<Lease*>(input->lease);
  if (!TryAcquireSRWLockExclusive(&owner->gate)) { BusyResult(*output, input->lease); return; }
  Invocation invocation(*owner,input->current);
  const bool retired = Call(invocation, *input, Cleanup) && invocation.Retire();
  owner->Result(*output); ReleaseSRWLockExclusive(&owner->gate);
  if (retired) {
    delete owner; output->lease = 0; output->retired = 1;
    output->live_handles = 0; output->live_locks = 0;
  }
}
