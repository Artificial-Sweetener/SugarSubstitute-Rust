// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <cstring>
#include <new>

namespace sugarsubstitute::finished {
void BusyResult(SsFinishedResultV1& output, uintptr_t lease) noexcept {
  // No mutable owner field is read without its gate. Busy carries conservative
  // retained capacity, invalid facts and no physical-retirement assertion.
  output = {}; output.abi_version = 1; output.status = Busy; output.lease = lease;
  output.live_handles = static_cast<uint32_t>(SlotCount) + 2;
  output.live_locks = 1;
}
SsFinishedStatusV1 Status(ParseStatus status) noexcept {
  switch (status) {
    case ParseStatus::Valid: return Finished;
    case ParseStatus::Missing: return Missing;
    case ParseStatus::Busy: return Busy;
    case ParseStatus::Corrupt: return Corrupt;
    case ParseStatus::WrongIdentity: return WrongIdentity;
    case ParseStatus::Oversized: return Oversized;
    case ParseStatus::DeadlineElapsed: return DeadlineElapsed;
    default: return Failed;
  }
}
bool ValidDeadline(const SsFinishedDeadlineV1& value) noexcept {
  return value.counter_deadline > 0 && value.counter_frequency > 0;
}
bool ValidBuild(uint32_t length, const char* build) noexcept {
  if (!length || length > 64) return false;
  for (uint32_t i = 0; i < 64; ++i) {
    const auto c = build[i];
    if (i < length) {
      if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
          (c >= '0' && c <= '9') || c == '-' || c == '_')) return false;
    } else if (c) return false;
  }
  return true;
}
bool ValidSubject(const SsFinishedSubjectV1& value) noexcept {
  if (value.reserved || std::all_of(std::begin(value.epoch), std::end(value.epoch), [](auto c) { return !c; })) return false;
  return value.tag == Supervisor ? !value.instance && !value.generation :
      value.tag == Desktop && value.instance && value.generation;
}
bool ValidOperation(const SsLogicalStoreOperationV1& value) noexcept {
  return value.id && std::any_of(std::begin(value.epoch), std::end(value.epoch), [](auto c) { return !!c; });
}
bool SameOperation(const SsLogicalStoreOperationV1& a, const SsLogicalStoreOperationV1& b) noexcept {
  return a.id == b.id && !memcmp(a.namespace_id, b.namespace_id, 32) && !memcmp(a.epoch, b.epoch, 16);
}
bool SameFacts(const SsCaptureFileFactsV1& a, const SsCaptureFileFactsV1& b) noexcept {
  return a.bytes == b.bytes && !memcmp(a.identity, b.identity, 24) && !memcmp(a.sha256, b.sha256, 32);
}
bool Lease::Fail(DWORD error, SsFinishedCauseOriginV1 origin) noexcept {
  if (!error) error = ERROR_GEN_FAILURE;
  if (!primary) {primary=error;primary_origin=origin;} else CleanupError(error,origin);
  status = Failed; return false;
}
void Lease::CleanupError(DWORD error, SsFinishedCauseOriginV1 origin) noexcept {
  if (!error) return;
  if (secondary_count < secondary.size()) {
    cleanup_origin_bits|=static_cast<uint32_t>(origin)<<(secondary_count*2);
    secondary[secondary_count++]=error;
  }
  else if (omitted != UINT64_MAX) ++omitted;
}
Checkpoint ObserveCurrent(const SsFinishedControlSourceV1& source) noexcept {
  if (!source.borrowed_state || !source.check) return {Interruption::ClockFailure,ERROR_INVALID_PARAMETER};
  uint32_t cause=0;
  switch(source.check(source.borrowed_state,&cause)) {
    case FinishedControlCurrent: return cause?Checkpoint{Interruption::ClockFailure,ERROR_INVALID_DATA}:Checkpoint{};
    case FinishedControlCancelled: return cause==ERROR_OPERATION_ABORTED?Checkpoint{Interruption::Cancelled,cause}:Checkpoint{Interruption::ClockFailure,ERROR_INVALID_DATA};
    case FinishedControlTimedOut: return cause==ERROR_TIMEOUT?Checkpoint{Interruption::DeadlineElapsed,cause}:Checkpoint{Interruption::ClockFailure,ERROR_INVALID_DATA};
    case FinishedControlFailed: return {Interruption::ClockFailure,cause?cause:ERROR_INVALID_DATA};
    default: return {Interruption::ClockFailure,ERROR_INVALID_DATA};
  }
}
Checkpoint ObserveCounter(FileApi& api,const SsFinishedDeadlineV1& cutoff,int64_t& last,int64_t frequency) noexcept {
  int64_t now=0,hz=0;const auto error=api.Clock(now,hz);
  if(error||now<last||now<0||hz!=frequency) return {Interruption::ClockFailure,error?error:ERROR_INVALID_DATA};
  last=now;
  if(!ValidDeadline(cutoff)||now>=cutoff.counter_deadline)return {Interruption::DeadlineElapsed,ERROR_TIMEOUT};
  return {};
}
bool Preflight(FileApi& api,const SsFinishedControlSourceV1& source,const SsFinishedDeadlineV1& cutoff,SsFinishedResultV1& output) noexcept {
  const auto control=ObserveCurrent(source);int64_t last=0;
  const auto counter=ObserveCounter(api,cutoff,last,cutoff.counter_frequency);
  const auto first=control.interruption!=Interruption::None?control:counter;
  if(first.interruption==Interruption::None)return true;
  const auto remember = [&](const Checkpoint& failure, SsFinishedCauseOriginV1 origin) noexcept {
    if(failure.interruption==Interruption::None)return;
    if(!output.primary_error) {
      output.primary_error=failure.cause;
      output.primary_origin=origin;
      output.status=failure.interruption==Interruption::DeadlineElapsed?DeadlineElapsed:Failed;
    } else if(output.cleanup_count<8) {
      output.cleanup_origin_bits|=static_cast<uint32_t>(origin)<<(output.cleanup_count*2);
      output.cleanup_codes[output.cleanup_count++]=failure.cause;
    }
    else if(output.omitted_cleanup!=UINT64_MAX) ++output.omitted_cleanup;
  };
  remember(control,FinishedCauseControl);remember(counter,FinishedCauseQpc);
  return false;
}
Lease* NativeLeaseAllocation(FileApi& api) noexcept { return new (std::nothrow) Lease(api); }
Lease* CreateLease(FileApi& api,const SsFinishedControlSourceV1& source,
                   const SsFinishedDeadlineV1& cutoff,SsFinishedResultV1& output,
                   AllocateLease allocate) noexcept {
  output={};output.abi_version=1;output.status=Failed;
  if(!Preflight(api,source,cutoff,output)) {
    // This actual branch has never called the allocator or acquired an output.
    output.retired=1;return nullptr;
  }
  auto owner=allocate(api);
  if(!owner) {
    output.primary_error=ERROR_NOT_ENOUGH_MEMORY;output.primary_origin=FinishedCauseNative;
    Preflight(api,source,cutoff,output); // retain independent post failures.
    output.retired=1;return nullptr;
  }
  owner->operation=cutoff;
  owner->Result(output); // Publish created custody before any native acquisition.
  return owner;
}
Checkpoint Invocation::Check() noexcept {
  const auto control=ObserveCurrent(control_source);
  if(control.interruption!=Interruption::None) {
    if(control.interruption==Interruption::DeadlineElapsed&&!state.primary){state.primary=control.cause;state.primary_origin=FinishedCauseControl;state.status=DeadlineElapsed;}
    else state.Fail(control.cause,FinishedCauseControl);
  }
  const auto counter=ObserveCounter(state.api,state.in_cleanup?state.cleanup:state.operation,state.last_counter,state.operation.counter_frequency);
  if(counter.interruption!=Interruption::None) {
    if(counter.interruption==Interruption::DeadlineElapsed&&!state.primary){state.primary=counter.cause;state.primary_origin=FinishedCauseQpc;state.status=DeadlineElapsed;}
    else state.Fail(counter.cause,FinishedCauseQpc);
  }
  echoed_stop=control.interruption!=Interruption::None?control:counter;
  return echoed_stop;
}
bool Invocation::Allowed() noexcept { return Check().interruption == Interruption::None; }
bool Invocation::Post(DWORD error) noexcept {
  // A primitive returning its already-latched pre-effect Check is not a new
  // native failure. An actual provider failure still precedes post observations.
  if (error && (echoed_stop.interruption==Interruption::None || error!=echoed_stop.cause)) state.Fail(error);
  // Always observe state.original control, including a failed provider return.
  const bool current = Allowed(); return !error && current;
}
ReadResult Invocation::Read(uint64_t offset, std::span<uint8_t> output) noexcept {
  if (offset > state.read_size || output.size() > state.read_size - offset || output.size() > 65536)
    return {false, ERROR_INVALID_PARAMETER};
  if (!Allowed()) return {false, state.primary};
  const auto error = state.api.Read(state.handles[state.read_slot], offset, output, *this);
  return {Post(error), error ? error : state.primary};
}
bool Invocation::Open(Slot slot, Slot parent, const wchar_t* leaf, OpenMode mode) noexcept {
  if (state.handles[slot] || !state.handles[parent]) return state.Fail(ERROR_INVALID_STATE);
  if (!Allowed()) return false;
  const auto error = state.api.Open(state.handles[parent], leaf, mode, state.handles[slot], *this);
  if (!state.handles[slot] && (error == ERROR_FILE_NOT_FOUND || error == ERROR_SHARING_VIOLATION || error == ERROR_LOCK_VIOLATION)) {
    const bool current = Allowed();
    if (current) state.status = error == ERROR_FILE_NOT_FOUND ? Missing : Busy;
    return false;
  }
  return Post(error);
}
bool Invocation::Close(Slot slot) noexcept {
  if (!state.handles[slot]) return true;
  if (!Allowed()) return false;
  const auto error = state.api.Close(state.handles[slot]);
  // Checked success retires that actual output even if post-control is stopped.
  if (!error) state.handles[slot] = nullptr;
  return Post(error);
}
bool Invocation::Info(Slot slot, Slot parent, FileInfo& output) noexcept {
  if (!state.handles[slot] || !state.handles[parent]) return state.Fail(ERROR_INVALID_HANDLE);
  if (!Allowed()) return false;
  return Post(state.api.Inspect(state.handles[slot], state.handles[parent], output, *this));
}
bool Invocation::Image(Slot slot, std::span<uint8_t> image, uint64_t bytes) noexcept {
  if (bytes > image.size()) return state.Fail(ERROR_INVALID_PARAMETER);
  for (uint64_t offset = 0; offset < bytes;) {
    const size_t count = static_cast<size_t>(std::min<uint64_t>(65536, bytes - offset));
    if (!Allowed() || !Post(state.api.Read(state.handles[slot], offset, image.subspan(static_cast<size_t>(offset), count), *this))) return false;
    offset += count;
  }
  return true;
}
bool Invocation::RetireDigest() noexcept {
  if (state.hash.hash && (!Allowed() || !Post(state.api.HashRetire(state.hash, true)))) return false;
  if (state.hash.algorithm && (!Allowed() || !Post(state.api.HashRetire(state.hash, false)))) return false;
  return true;
}
bool Invocation::Digest(Slot slot, uint64_t bytes, uint8_t* output) noexcept {
  if (!Allowed() || !Post(state.api.HashBegin(state.hash, *this))) return false;
  for (uint64_t offset = 0; offset < bytes;) {
    const size_t count = static_cast<size_t>(std::min<uint64_t>(65536, bytes - offset));
    if (!Allowed() || !Post(state.api.Read(state.handles[slot], offset, std::span(state.scratch).first(count), *this)) ||
        !Allowed() || !Post(state.api.HashUpdate(state.hash, std::span(state.scratch).first(count)))) return false;
    offset += count;
  }
  if (!Allowed() || !Post(state.api.HashFinish(state.hash, std::span<uint8_t, 32>(output, 32)))) return false;
  if (!Allowed()) return false;
  return RetireDigest();
}
bool Invocation::DigestBytes(std::span<const uint8_t> bytes, uint8_t* output) noexcept {
  if (!Allowed() || !Post(state.api.HashBegin(state.hash, *this))) return false;
  for (size_t offset = 0; offset < bytes.size();) {
    const auto count = std::min<size_t>(65536, bytes.size() - offset);
    if (!Allowed() || !Post(state.api.HashUpdate(state.hash, bytes.subspan(offset, count)))) return false;
    offset += count;
  }
  if (!Allowed() || !Post(state.api.HashFinish(state.hash, std::span<uint8_t, 32>(output, 32)))) return false;
  return RetireDigest();
}
bool Invocation::FileFacts(Slot slot, Slot parent, uint64_t limit, SsCaptureFileFactsV1& output) noexcept {
  FileInfo info{};
  if (!Info(slot, parent, info)) return false;
  if (info.directory || !info.bytes || info.bytes > limit) { state.status = info.bytes > limit ? Oversized : Corrupt; return false; }
  memcpy(output.identity, info.identity.data(), 24); output.bytes = info.bytes;
  return Digest(slot, info.bytes, output.sha256);
}
bool Invocation::LockMetadata(bool exclusive) noexcept {
  if (state.locked || !state.handles[Metadata]) return state.Fail(ERROR_INVALID_STATE);
  if (!Allowed()) return false;
  const auto error = state.api.Lock(state.handles[Metadata], exclusive);
  if (!error) state.locked = true;
  if (error == ERROR_LOCK_VIOLATION || error == ERROR_SHARING_VIOLATION) {
    state.status = Busy; Allowed(); return false;
  }
  return Post(error);
}
bool Invocation::UnlockMetadata() noexcept {
  if (!state.locked) return true;
  if (!Allowed()) return false;
  const auto error = state.api.Unlock(state.handles[Metadata]);
  if (!error) state.locked = false;
  return Post(error);
}
void Lease::Result(SsFinishedResultV1& output) noexcept {
  output = {}; output.abi_version = 1; output.status = status; output.primary_error = primary;
  output.primary_origin=primary_origin;output.cleanup_origin_bits=cleanup_origin_bits;
  output.cleanup_count = secondary_count; std::copy(secondary.begin(), secondary.end(), output.cleanup_codes);
  output.omitted_cleanup = omitted; output.lease = reinterpret_cast<uintptr_t>(this);
  output.live_handles = static_cast<uint32_t>(std::count_if(handles.begin(), handles.end(), [](auto h) { return !!h; })) +
      (hash.algorithm ? 1u : 0u) + (hash.hash ? 1u : 0u);
  output.live_locks = locked ? 1u : 0u; output.facts_valid = facts_valid ? 1u : 0u;
  if (facts_valid) output.facts = facts;
  output.actual_read_bytes = read_bytes; output.actual_reclaimed_bytes = reclaimed;
}

bool RecoverySubject(const SsRecoveryCaptureOriginV2& origin,const SsFinishedSubjectV1& subject) noexcept {
  if (!ValidSubject(subject) || memcmp(origin.namespace_id,subject.namespace_id,32)) return false;
  if (origin.origin_kind==1) return !memcmp(origin.root_epoch,subject.epoch,16);
  if (origin.origin_kind!=2) return false;
  if (subject.tag==Desktop) return !memcmp(origin.root_epoch,subject.epoch,16) &&
      subject.instance==origin.instance && subject.generation==origin.generation;
  return subject.tag==Supervisor && !memcmp(origin.capture_epoch,subject.epoch,16);
}
bool RecoveryOperation(const SsRecoveryCaptureOriginV2& origin,const SsLogicalStoreOperationV1& operation) noexcept {
  // Matching data only. Rust supplies a genuine current operation and a protected-read
  // bound origin before this private ABI; historical Root epochs are not current authority.
  return ValidOperation(operation) && !memcmp(origin.namespace_id,operation.namespace_id,32);
}
bool BindRecoveryOrigin(Invocation& invocation,const SsRecoveryCaptureOriginV2& origin,
                        const SsFinishedSubjectV1& subject,const SsLogicalStoreOperationV1& operation) noexcept {
  auto& owner=invocation.state;
  if (owner.recovery_origin_bound || origin.reserved ||
      std::all_of(std::begin(origin.namespace_id),std::end(origin.namespace_id),[](auto b){return !b;}) || (origin.origin_kind!=1 && origin.origin_kind!=2) ||
      (origin.origin_kind==1 && (origin.instance || origin.generation || origin.grant || memcmp(origin.capture_epoch,origin.root_epoch,16))) ||
      (origin.origin_kind==2 && (!origin.instance || !origin.generation || !origin.grant)) ||
      std::all_of(std::begin(origin.root_epoch),std::end(origin.root_epoch),[](auto b){return !b;}) ||
      std::all_of(std::begin(origin.capture_epoch),std::end(origin.capture_epoch),[](auto b){return !b;}) ||
      !RecoverySubject(origin,subject) || !RecoveryOperation(origin,operation)) return owner.Fail(ERROR_INVALID_DATA);
  if (origin.origin_kind==1) {
    owner.recovery_origin=origin;owner.recovery_current_operation=operation;owner.recovery_origin_bound=true;return true;
  }
  constexpr char domain[]="SugarSubstitute.recovery-supervisor.capture.v1";
  std::array<uint8_t,sizeof(domain)+48+24> bytes{};
  memcpy(bytes.data(),domain,sizeof(domain));
  memcpy(bytes.data()+sizeof(domain),origin.namespace_id,32);
  memcpy(bytes.data()+sizeof(domain)+32,origin.root_epoch,16);
  const uint64_t fields[]={origin.instance,origin.generation,origin.grant};
  for (size_t field=0;field<3;++field) for(size_t byte=0;byte<8;++byte)
    bytes[sizeof(domain)+48+field*8+byte]=static_cast<uint8_t>(fields[field]>>(byte*8));
  std::array<uint8_t,32> digest{};
  if (!invocation.DigestBytes(bytes,digest.data())) return false;
  if (memcmp(digest.data(),origin.capture_epoch,16)) return owner.Fail(ERROR_INVALID_DATA);
  owner.recovery_origin=origin; owner.recovery_current_operation=operation; owner.recovery_origin_bound=true;
  return true;
}
}  // namespace sugarsubstitute::finished
