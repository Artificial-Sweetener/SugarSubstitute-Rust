// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/owned_attach.h"
#include <limits>
#include <string.h>

namespace sugarsubstitute {
namespace {
// No borrowed user callback/context escapes this synchronous scope. TLS binds
// nested Step to the original worker without a kernel query under the fence.
thread_local OwnedAttach* active_call = nullptr;
struct ActiveCall {
  OwnedAttach* previous;
  bool& in_call;
  ActiveCall(OwnedAttach* owner, bool& flag) : previous(active_call), in_call(flag) {
    active_call = owner;
    in_call = true;
  }
  ~ActiveCall() { in_call = false; active_call = previous; }
};
}  // namespace

bool OwnedAttach::Begin(const SsCaptureAttachInputV2& input) {
  if (started || !ValidAttachInput(input) || !runtime ||
      !runtime->initialize || !runtime->inspect_event ||
      !runtime->validate_events || !runtime->install_handlers)
    return connection.Fail(ERROR_INVALID_PARAMETER);
  started = true;
  worker_thread = GetCurrentThreadId();
  creator = reinterpret_cast<HANDLE>(input.creator_process);
  handler = reinterpret_cast<HANDLE>(input.handler_process);
  creator_pid = input.creator_pid;
  handler_pid = input.handler_pid;
  creator_birth = input.creator_birth;
  handler_birth = input.handler_birth;
  memcpy(endpoint, input.endpoint, input.endpoint_length * sizeof(uint16_t));
  memcpy(build, input.build, input.build_length);
  memcpy(coordinator_namespace, input.coordinator_namespace, sizeof(coordinator_namespace));
  memcpy(coordinator_epoch, input.coordinator_epoch, sizeof(coordinator_epoch));
  instance = input.instance;
  generation = input.generation;
  subject = input.subject;
  connection.deadline_counter = input.attach_counter;
  connection.frequency = input.counter_frequency;
  connection.event = reinterpret_cast<HANDLE>(input.borrowed_completion_event);
  completion_event = connection.event;
  connection.event_custody = EventCustody::Borrowed;
  connection.overlapped.hEvent = connection.event;
  return true;
}
bool OwnedAttach::Accept(AttachStage accepted) {
  if (accepted == AttachStage::Issue) return Issue();
  if (accepted == AttachStage::Publish) {
    if (!globals_committed || !handlers_started || !response_complete)
      return connection.Fail(ERROR_INVALID_STATE);
    registered = true;
    stage = AttachStage::Done;
  }
  return true;
}
bool OwnedAttach::Step(const SsCaptureAttachStepV2& request) {
  if (!ValidAttachStep(request) || active_call != this || !call_active_ || claim_used_ ||
      request.attach != reinterpret_cast<uintptr_t>(this) ||
      request.stage != static_cast<uint32_t>(stage) || request.nonce != nonce ||
      stopped || expired || connection.error || retired)
    return false;
  claim_used_ = true;
  // Every output slot/message already belongs to this process-pinned owner.
  // The only external operation here is nonblocking TransactNamedPipe acceptance.
  return Accept(stage);
}
void OwnedAttach::Poll(const SsCaptureAttachProgressV2& progress) {
  if (!ValidAttachProgress(progress) || progress.attach != reinterpret_cast<uintptr_t>(this) ||
      !started || GetCurrentThreadId() != worker_thread || call_active_ || retired) {
    connection.Fail(ERROR_INVALID_PARAMETER);
    return;
  }
  if (stopped || expired || connection.error || stage == AttachStage::Done) return;
  // All counter/process/completion queries precede the borrowed issue callback.
  if (!connection.CheckDeadline()) { expired = true; return; }
  if (connection.pending) { Complete(); return; }
  if (!ValidateProcesses()) return;
  if (nonce == std::numeric_limits<uint64_t>::max()) {
    connection.Fail(ERROR_ARITHMETIC_OVERFLOW); return;
  }
  ++nonce;
  const AttachStage requested = stage;
  claim_used_ = false;
  uint32_t decision = 4;
  {
    ActiveCall active(this, call_active_);
    // Rust catches its own unwind before crossing this ABI; no owner lock exists.
    decision = progress.issue.request(progress.issue.borrowed_context,
                                     reinterpret_cast<uintptr_t>(this),
                                     static_cast<uint32_t>(requested), nonce);
  }
  if (decision != 1 || !claim_used_) {
    if (decision == 2) stopped = true;
    else if (decision == 3) expired = true;
    else connection.Fail(ERROR_OPERATION_ABORTED);
    return;
  }
  if (connection.error || stage == AttachStage::Done) return;
  // A finite claim is permission for this one retained stage only. Its actual
  // calls run after the callback has released Core/custody, on this same worker.
  bool complete = false;
  switch (requested) {
    case AttachStage::Open: complete = Open(); break;
    case AttachStage::Initialize: complete = runtime->initialize(this); break;
    case AttachStage::Issue: return;
    case AttachStage::Validate:
      complete = AdoptResponse() && InspectEvents(nullptr) && runtime->validate_events(this);
      break;
    case AttachStage::Install:
      handlers_started = true;
      complete = runtime->install_handlers(this);
      break;
    case AttachStage::Publish:
    case AttachStage::Done: return;
  }
  if (!complete) {
    if (!connection.error) connection.Fail(ERROR_GEN_FAILURE);
    return;
  }
  stage = static_cast<AttachStage>(static_cast<uint32_t>(requested) + 1);
  // No second stage/callback follows a finite boundary in this poll. Eligibility
  // is freshly checked by the original Execution owner at the next callback.
}
bool OwnedAttach::LatchCleanup(const SsCaptureCleanupSourceV1& source) {
  if (cleanup.state) return true;
  SsCaptureCleanupBudgetV1 observed = {};
  if (!source.request(source.borrowed_context, &observed)) return false;
  if (observed.abi_version != 1 || observed.byte_size != sizeof(observed) ||
      observed.state != 1 || observed.reserved || observed.start_counter < 0 ||
      observed.counter_frequency != connection.frequency ||
      observed.grace_counter <= observed.start_counter ||
      observed.total_counter <= observed.grace_counter)
    return connection.Fail(ERROR_INVALID_DATA);
  cleanup = observed;
  cleanup_limit.start_counter = observed.start_counter;
  cleanup_limit.total_counter = observed.total_counter;
  cleanup_limit.previous_counter = observed.start_counter;
  return true;
}
void OwnedAttach::Retire(const SsCaptureAttachProgressV2& progress) {
  if (!ValidAttachProgress(progress) || progress.attach != reinterpret_cast<uintptr_t>(this) ||
      !started || GetCurrentThreadId() != worker_thread || call_active_) {
    connection.Fail(ERROR_INVALID_PARAMETER); return;
  }
  stopped = true;
  if (retired || !LatchCleanup(progress.cleanup)) return;
  const bool connection_closed = connection.Retire(&cleanup_limit);
  if (!connection.pending) RecordResponse();
  // Cleanup inspection grants only checked closure of actual returned outputs;
  // it never commits globals or grants a stopped main a new issue stage.
  const bool events_closed = !connection.pending && InspectEvents(&cleanup_limit) && CloseEvents();
  retired = connection_closed && events_closed;
}
void OwnedAttach::Facts(SsCaptureAttachResultV2* output) {
  if (!output) return;
  *output = {};
  output->abi_version = 2;
  output->status = retired ? 4 : registered ? 1 : stopped ? 3 :
                   connection.error || expired ? 2 : started ? 5 : 0;
  output->registered = registered ? 1 : 0;
  output->pending_io = connection.pending ? 1 : 0;
  output->retired = retired ? 1 : 0;
  output->primary_error = connection.error;
  output->primary_error_domain = connection.error ? static_cast<uint32_t>(connection.error_domain) : 0;
  output->attach = started ? reinterpret_cast<uintptr_t>(this) : 0;
  output->cleanup_count = static_cast<uint32_t>(connection.cleanup_count);
  for (size_t i = 0; i < connection.cleanup_count; ++i) {
    output->cleanup_codes[i] = connection.cleanup_codes[i];
    output->cleanup_error_domains |= static_cast<uint32_t>(connection.cleanup_domains[i]) << (i * 2);
  }
  output->omitted_cleanup = connection.omitted_cleanup;
  connection.cleanup_count = 0;
  connection.omitted_cleanup = 0;
  output->cleanup_state = cleanup.state;
  output->cleanup_start = cleanup.start_counter;
  output->cleanup_grace = cleanup.grace_counter;
  output->cleanup_total = cleanup.total_counter;
  output->counter_frequency = connection.frequency;
}
}  // namespace sugarsubstitute
