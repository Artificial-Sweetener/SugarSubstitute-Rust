// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <cstring>
#include <new>

namespace sugarsubstitute::finished {
namespace {
bool Nonzero(const uint8_t* bytes, size_t size) noexcept {
  return std::any_of(bytes, bytes + size, [](auto b) { return !!b; });
}
bool Facts(const SsCaptureFileFactsV1& facts, uint64_t maximum) noexcept {
  return facts.bytes && facts.bytes <= maximum && Nonzero(facts.identity, 24) && Nonzero(facts.sha256, 32);
}
bool WriteImage(Invocation& invocation, Slot slot, std::span<const uint8_t> image) noexcept {
  auto& owner = invocation.state;
  for (size_t offset = 0; offset < image.size();) {
    const auto count = std::min<size_t>(65536, image.size() - offset);
    if (!invocation.Allowed() || !invocation.Post(owner.api.Write(owner.handles[slot], offset, image.subspan(offset, count), invocation))) return false;
    offset += count;
  }
  if (!invocation.Allowed() || !invocation.Post(owner.api.Truncate(owner.handles[slot], image.size(), invocation)) ||
      !invocation.Allowed() || !invocation.Post(owner.api.Flush(owner.handles[slot]))) return false;
  for (size_t offset = 0; offset < image.size();) {
    const auto count = std::min<size_t>(65536, image.size() - offset);
    if (!invocation.Allowed() || !invocation.Post(owner.api.Read(owner.handles[slot], offset, std::span(owner.scratch).first(count), invocation))) return false;
    if (!std::equal(owner.scratch.begin(), owner.scratch.begin() + count, image.begin() + offset)) return owner.Fail(ERROR_INVALID_DATA);
    offset += count;
  }
  return true;
}
bool BuildCandidateImage(Invocation& invocation, const SsCapturePreparationV1& preparation) noexcept {
  auto& owner = invocation.state;
  MetadataFacts parsed{};
  const auto result = ParseCrashpadMetadata(std::span(owner.original).first(owner.metadata_bytes), invocation, parsed);
  if (result.status != ParseStatus::Valid) { owner.status = Status(result.status); return false; }
  Uuid uuid{}; memcpy(uuid.data(), preparation.uuid, 16); uint32_t index = 0;
  const auto found = FindMetadataUuid(parsed, uuid, index);
  if (found.status != ParseStatus::Valid) { owner.status = found.status == ParseStatus::Busy ? Busy : WrongIdentity; return false; }
  const auto offset = parsed.records[index].offset;
  owner.candidate_bytes = owner.metadata_bytes - kMetadataRecordBytes;
  std::copy_n(owner.original.begin(), offset, owner.candidate.begin());
  std::copy(owner.original.begin() + offset + kMetadataRecordBytes, owner.original.begin() + owner.metadata_bytes, owner.candidate.begin() + offset);
  const auto count = parsed.count - 1;
  for (size_t i = 0; i < 4; ++i) owner.candidate[8 + i] = static_cast<uint8_t>(count >> (8 * i));
  // Records keep their string-table-relative offsets. No unrelated bytes change.
  return true;
}
bool ValidatePermit(Invocation& invocation, const SsCaptureDispositionV1& permit) noexcept {
  auto& owner = invocation.state;
  const auto& p = permit.prepared;
  if (p.abi_version != 1 || p.reserved || p.reserved_build ||
      !ValidSubject(p.subject) || !ValidBuild(p.build_length, p.build) ||
      !ValidOperation(p.historical_operation) || !ValidOperation(permit.current_operation) ||
      !Nonzero(p.uuid, 16) || memcmp(p.historical_operation.namespace_id, p.subject.namespace_id, 32) ||
      (owner.recovery_origin_bound ?
          (!RecoverySubject(owner.recovery_origin,p.subject) || !RecoveryOperation(owner.recovery_origin,p.historical_operation) ||
           !SameOperation(owner.recovery_current_operation,permit.current_operation)) :
          memcmp(p.historical_operation.epoch, p.subject.epoch, 16)) ||
      memcmp(permit.current_operation.namespace_id, p.subject.namespace_id, 32) ||
      !Facts(p.source, kDumpLimit) || !Facts(p.metadata_preimage, kMetadataLimit) ||
      !Facts(p.original_stage.file, kMetadataLimit) || !Facts(p.candidate_stage.file, kMetadataLimit) ||
      p.original_stage.role != OriginalMetadata || p.candidate_stage.role != CandidateMetadata ||
      p.original_stage.reserved || p.candidate_stage.reserved ||
      p.original_stage.file.bytes != p.metadata_preimage.bytes ||
      memcmp(p.original_stage.file.sha256, p.metadata_preimage.sha256, 32) ||
      p.metadata_preimage.bytes < kMetadataHeaderBytes + kMetadataRecordBytes ||
      p.candidate_stage.file.bytes != p.metadata_preimage.bytes - kMetadataRecordBytes ||
      !memcmp(p.original_stage.file.identity, p.candidate_stage.file.identity, 24) ||
      !memcmp(p.original_stage.file.identity, p.metadata_preimage.identity, 24) ||
      !memcmp(p.candidate_stage.file.identity, p.metadata_preimage.identity, 24) ||
      !memcmp(p.candidate_stage.file.sha256, p.original_stage.file.sha256, 32) ||
      !Facts(permit.committed_summary, 65536) || !Facts(permit.committed_intent, 65536) ||
      !Nonzero(permit.canonical_plan_sha256, 32) || memcmp(permit.canonical_plan_sha256, permit.committed_intent.sha256, 32) || !permit.incident_revision ||
      permit.reserved_payload || permit.reserved_authorization || permit.has_payload > 1) return owner.Fail(ERROR_INVALID_DATA);
  if (permit.disposition == 1) {
    if (permit.discard_reason || !permit.has_payload || !Facts(permit.committed_payload, kDumpLimit) ||
        permit.committed_payload.bytes != p.source.bytes ||
        memcmp(permit.committed_payload.sha256, p.source.sha256, 32)) return owner.Fail(ERROR_INVALID_DATA);
  } else if (permit.disposition == 2) {
    if (permit.discard_reason < 1 || permit.discard_reason > 4 || permit.has_payload ||
        Nonzero(reinterpret_cast<const uint8_t*>(&permit.committed_payload), sizeof(permit.committed_payload))) return owner.Fail(ERROR_INVALID_DATA);
  } else return owner.Fail(ERROR_INVALID_DATA);
  if (owner.recovering) {
    if (permit.authorization != RecoveryDisposition || SameOperation(permit.current_operation, p.historical_operation)) return owner.Fail(ERROR_INVALID_STATE);
  } else if (permit.authorization != NormalDisposition || !SameOperation(permit.current_operation, p.historical_operation) ||
      !owner.prepared || memcmp(&p, &owner.preparation, sizeof(p))) return owner.Fail(ERROR_INVALID_STATE);
  return true;
}
bool ExactFile(Invocation& invocation, Slot slot, Slot parent, const SsCaptureFileFactsV1& expected,
               uint64_t bound) noexcept {
  auto& owner = invocation.state;
  SsCaptureFileFactsV1 actual{};
  if (!invocation.FileFacts(slot, parent, bound, actual)) return false;
  if (!SameFacts(actual, expected)) { owner.status = WrongIdentity; return false; }
  return true;
}
bool DeleteAndAbsent(Invocation& invocation, Slot slot, Slot parent, const wchar_t* name) noexcept {
  auto& owner = invocation.state;
  if (!invocation.Allowed() || !invocation.Post(owner.api.Dispose(owner.handles[slot]))) return false;
  return invocation.Close(slot) && invocation.Absent(parent, name);
}
bool OpenMaybe(Invocation& invocation, Slot slot, Slot parent, const wchar_t* name, OpenMode mode, bool& absent) noexcept {
  auto& owner = invocation.state;
  if (!invocation.Allowed() || owner.handles[slot]) return false;
  const auto error = owner.api.Open(owner.handles[parent], name, mode, owner.handles[slot], invocation);
  const bool current = invocation.Allowed();
  if (error == ERROR_FILE_NOT_FOUND && !owner.handles[slot]) { absent = true; return current; }
  return invocation.Post(error) && current;
}
}  // namespace
bool StageNames(Invocation& invocation, const SsLogicalStoreOperationV1& key) noexcept {
  auto& owner = invocation.state;
  std::array<uint8_t, 56> bytes{}; memcpy(bytes.data(), key.namespace_id, 32); memcpy(bytes.data() + 32, key.epoch, 16);
  for (size_t i = 0; i < 8; ++i) bytes[48 + i] = static_cast<uint8_t>(key.id >> (8 * i));
  std::array<uint8_t, 32> digest{};
  if (!invocation.DigestBytes(bytes, digest.data())) return false;
  constexpr wchar_t hex[] = L"0123456789abcdef";
  for (size_t role = 0; role < 2; ++role) {
    auto& name = owner.stage_names[role]; std::copy_n(L"ss-", 3, name.begin());
    for (size_t i = 0; i < digest.size(); ++i) { name[3 + 2 * i] = hex[digest[i] >> 4]; name[4 + 2 * i] = hex[digest[i] & 15]; }
    std::copy_n(role ? L"-c.stage" : L"-o.stage", 9, name.begin() + 67);
  }
  return true;
}
bool Prepare(Invocation& invocation, const SsLogicalStoreOperationV1& operation) noexcept {
  auto& owner = invocation.state;
  if (!owner.facts_valid || owner.prepared || owner.primary || !ValidOperation(operation) ||
      memcmp(operation.namespace_id, owner.facts.subject.namespace_id, 32) ||
      (owner.recovery_origin_bound ? !SameOperation(owner.recovery_current_operation,operation) :
          memcmp(operation.epoch,owner.facts.subject.epoch,16))) return owner.Fail(ERROR_INVALID_STATE);
  auto& p = owner.preparation; p.abi_version = 1; p.historical_operation = operation;
  p.subject = owner.facts.subject; memcpy(p.uuid, owner.facts.uuid, 16);
  p.build_length = owner.facts.build_length; memcpy(p.build, owner.facts.build, 64);
  memcpy(p.database_identity, owner.expected.expected_database_identity, 24);
  memcpy(p.metrics_identity, owner.expected.expected_metrics_identity, 24);
  if (!invocation.LockMetadata(true) || !invocation.FileFacts(Metadata, Database, kMetadataLimit, p.metadata_preimage)) return false;
  owner.metadata_bytes = static_cast<uint32_t>(p.metadata_preimage.bytes);
  if (!invocation.Image(Metadata, owner.original, p.metadata_preimage.bytes) || !BuildCandidateImage(invocation, p) ||
      !invocation.FileFacts(Source, Reports, kDumpLimit, p.source) ||
      p.source.bytes != owner.facts.source_bytes || memcmp(p.source.identity, owner.facts.source_file_identity, 24)) return owner.Fail(ERROR_INVALID_DATA);
  if (!StageNames(invocation, operation) ||
      !invocation.Open(OriginalStage, Database, owner.stage_names[0].data(), OpenMode::CreateStage) ||
      !WriteImage(invocation, OriginalStage, std::span(owner.original).first(owner.metadata_bytes)) ||
      !invocation.FileFacts(OriginalStage, Database, kMetadataLimit, p.original_stage.file)) return false;
  p.original_stage.role = OriginalMetadata;
  if (!invocation.Open(CandidateStage, Database, owner.stage_names[1].data(), OpenMode::CreateStage) ||
      !WriteImage(invocation, CandidateStage, std::span(owner.candidate).first(owner.candidate_bytes)) ||
      !invocation.FileFacts(CandidateStage, Database, kMetadataLimit, p.candidate_stage.file)) return false;
  p.candidate_stage.role = CandidateMetadata;
  if (memcmp(p.original_stage.file.sha256, p.metadata_preimage.sha256, 32) ||
      !memcmp(p.original_stage.file.identity, p.candidate_stage.file.identity, 24)) return owner.Fail(ERROR_INVALID_DATA);
  if (!invocation.UnlockMetadata()) return false;
  const auto temporary = p.original_stage.file.bytes + p.candidate_stage.file.bytes;
  if (temporary > UINT64_MAX - owner.facts.observed_temporary_bytes) return owner.Fail(ERROR_ARITHMETIC_OVERFLOW);
  owner.facts.observed_temporary_bytes += temporary;
  if (owner.facts.observed_entries > 4094) owner.facts.inventory_complete = 0;
  else owner.facts.observed_entries += 2;
  owner.prepared = true; owner.status = Finished; return true;
}
bool Disposition(Invocation& invocation, const SsCaptureDispositionV1& permit) noexcept {
  auto& owner = invocation.state;
  if (owner.primary || !ValidatePermit(invocation, permit)) return false;
  // Retain the exact immutable intent on first use; a changed replay rejects.
  if (owner.disposition.authorization && memcmp(&owner.disposition, &permit, sizeof(permit))) return owner.Fail(ERROR_INVALID_STATE);
  owner.disposition = permit;
  const auto& p = permit.prepared;
  if (!owner.locked && !invocation.LockMetadata(true)) return false;
  SsCaptureFileFactsV1 metadata{};
  if (!invocation.FileFacts(Metadata, Database, kMetadataLimit, metadata)) return false;
  if (memcmp(metadata.identity, p.metadata_preimage.identity, 24)) { owner.status = WrongIdentity; return false; }
  const bool original = metadata.bytes == p.metadata_preimage.bytes && !memcmp(metadata.sha256, p.metadata_preimage.sha256, 32);
  const bool candidate = metadata.bytes == p.candidate_stage.file.bytes && !memcmp(metadata.sha256, p.candidate_stage.file.sha256, 32);
  if (!original && !candidate) { owner.metadata_state = MetadataUncertain; owner.status = Corrupt; return false; }
  owner.metadata_state = candidate ? CandidateImage : OriginalImage;
  if (original && owner.source_absent) { owner.status = WrongIdentity; return false; }
  if (!owner.source_absent) {
    if (!owner.handles[OriginalStage] || !owner.handles[CandidateStage] ||
        !ExactFile(invocation, OriginalStage, Database, p.original_stage.file, kMetadataLimit) ||
        !ExactFile(invocation, CandidateStage, Database, p.candidate_stage.file, kMetadataLimit) ||
        !ExactFile(invocation, Source, Reports, p.source, kDumpLimit)) return false;
    owner.metadata_bytes = static_cast<uint32_t>(p.metadata_preimage.bytes);
    if (!invocation.Image(OriginalStage, owner.original, p.original_stage.file.bytes) || !BuildCandidateImage(invocation, p)) return false;
    uint8_t calculated[32]{};
    if (!invocation.DigestBytes(std::span(owner.candidate).first(owner.candidate_bytes), calculated)) return false;
    if (memcmp(calculated, p.candidate_stage.file.sha256, 32)) return owner.Fail(ERROR_INVALID_DATA);
    if (original && !WriteImage(invocation, Metadata, std::span(owner.candidate).first(owner.candidate_bytes))) {
      owner.metadata_state = MetadataUncertain; return false;
    }
    owner.metadata_state = CandidateImage;
    if (!DeleteAndAbsent(invocation, Source, Reports, owner.source_name.data())) { owner.source_state = SourceUncertain; return false; }
    owner.source_absent = true; owner.source_state = DeletedNow;
  } else owner.source_state = PriorDeletionReconciled;
  for (size_t i = 0; i < 2; ++i) {
    const auto slot = i ? CandidateStage : OriginalStage;
    if (owner.handles[slot]) {
      const auto& expected = i ? p.candidate_stage.file : p.original_stage.file;
      if (!ExactFile(invocation, slot, Database, expected, kMetadataLimit) ||
          !DeleteAndAbsent(invocation, slot, Database, owner.stage_names[i].data())) return false;
    } else if (!invocation.Absent(Database, owner.stage_names[i].data())) return false;
    owner.absent_mask |= 1u << i;
  }
  if (!invocation.UnlockMetadata()) return false;
  owner.disposed = true;
  // Only checked zero handles/locks is a settled source-disposition result.
  if (!invocation.Retire()) return false;
  owner.reclaimed = owner.source_state == DeletedNow ? p.source.bytes : 0;
  owner.status = Finished; return true;
}
bool Recover(Invocation& invocation, const SsFinishedRecoverInputV1& input) noexcept {
  auto& owner = invocation.state;
  owner.recovering = true;
  const auto& permit = *input.borrowed_live_recovery_permit;
  if (!ValidatePermit(invocation, permit)) return false;
  owner.preparation = permit.prepared;
  if (memcmp(input.expected_database_identity, permit.prepared.database_identity, 24) ||
      memcmp(input.expected_metrics_identity, permit.prepared.metrics_identity, 24)) return owner.Fail(ERROR_INVALID_DATA);
  if (!invocation.Allowed() || !invocation.Post(owner.api.Duplicate(reinterpret_cast<HANDLE>(input.borrowed_database), owner.handles[Database])) ||
      !invocation.Allowed() || !invocation.Post(owner.api.Duplicate(reinterpret_cast<HANDLE>(input.borrowed_metrics), owner.handles[Metrics]))) return false;
  FileInfo db{}, metrics{};
  if (!invocation.Info(Database, Database, db) || !invocation.Info(Metrics, Metrics, metrics)) return false;
  if (!db.directory || !metrics.directory || memcmp(db.identity.data(), input.expected_database_identity, 24) ||
      memcmp(metrics.identity.data(), input.expected_metrics_identity, 24)) return owner.Fail(ERROR_ACCESS_DENIED);
  if (!invocation.Open(Reports, Database, L"reports", OpenMode::Directory) ||
      !invocation.Open(Metadata, Database, L"metadata", OpenMode::Metadata) ||
      !StageNames(invocation, permit.prepared.historical_operation)) return false;
  constexpr wchar_t hex[] = L"0123456789abcdef"; size_t cursor = 0;
  for (size_t i = 0; i < 16; ++i) {
    if (i == 4 || i == 6 || i == 8 || i == 10) owner.source_name[cursor++] = L'-';
    owner.source_name[cursor++] = hex[permit.prepared.uuid[i] >> 4];
    owner.source_name[cursor++] = hex[permit.prepared.uuid[i] & 15];
  }
  std::copy_n(L".dmp", 5, owner.source_name.begin() + 36);
  if (!OpenMaybe(invocation, Source, Reports, owner.source_name.data(), OpenMode::Read, owner.source_absent)) return false;
  for (size_t i = 0; i < 2; ++i) {
    bool absent = false;
    if (!OpenMaybe(invocation, i ? CandidateStage : OriginalStage, Database, owner.stage_names[i].data(), OpenMode::RecoverStage, absent)) return false;
  }
  return Disposition(invocation, permit);
}
}  // namespace sugarsubstitute::finished

using namespace sugarsubstitute::finished;
namespace {
bool OperationCall(Invocation& invocation, const SsFinishedCallV1& call) noexcept {
  auto& owner = invocation.state;
  if (call.abi_version != 1 || call.phase != Operation || !ValidDeadline(call.original) || owner.primary ||
      call.original.counter_frequency != owner.operation.counter_frequency ||
      call.original.counter_deadline > owner.operation.counter_deadline) return owner.Fail(ERROR_INVALID_PARAMETER);
  owner.in_cleanup = false; owner.operation.counter_deadline = call.original.counter_deadline;
  return invocation.Allowed();
}
void DispositionResult(Lease* owner, bool settled, SsFinishedDispositionResultV1& output) noexcept {
  owner->Result(output.state); output.source_state = owner->source_state;
  output.metadata_state = owner->metadata_state; output.stages_absent_mask = owner->absent_mask;
  if (settled) { output.state.lease = 0; output.state.retired = 1; output.state.live_handles = 0; output.state.live_locks = 0; }
}
}
void ss_finished_prepare_disposition_v1(const SsFinishedPrepareInputV1* input, SsFinishedPrepareResultV1* output) {
  if (!output) return;
  *output = {}; output->state.abi_version = 1; output->state.status = Failed; output->state.primary_error = ERROR_INVALID_PARAMETER;
  output->state.primary_origin=FinishedCauseNative;
  if (!input || !input->call.lease) return;
  auto& owner = *reinterpret_cast<Lease*>(input->call.lease);
  if (!TryAcquireSRWLockExclusive(&owner.gate)) { BusyResult(output->state, input->call.lease); return; }
  Invocation invocation(owner,input->call.current);
  if (OperationCall(invocation, input->call) && Prepare(invocation, input->operation)) {
    output->preparation = owner.preparation; output->preparation_valid = 1;
  }
  owner.Result(output->state); ReleaseSRWLockExclusive(&owner.gate);
}
static void DiscardOwned(const SsFinishedDiscardInputV1* input, const SsRecoveryCaptureOriginV2* origin, SsFinishedDispositionResultV1* output) {
  if (!output) return;
  *output = {}; output->state.abi_version = 1; output->state.status = Failed; output->state.primary_error = ERROR_INVALID_PARAMETER;
  output->state.primary_origin=FinishedCauseNative;
  output->source_state=SourceUncertain;output->metadata_state=MetadataUncertain;
  if (!input || !input->call.lease || !input->borrowed_live_permit) return;
  auto owner = reinterpret_cast<Lease*>(input->call.lease);
  if (!TryAcquireSRWLockExclusive(&owner->gate)) { BusyResult(output->state, input->call.lease); return; }
  owner->read_bytes = 0;
  Invocation invocation(*owner,input->call.current);
  const bool bound = origin ? owner->recovery_origin_bound && !memcmp(origin,&owner->recovery_origin,sizeof(*origin)) : !owner->recovery_origin_bound;
  const bool settled = bound && OperationCall(invocation,input->call) && Disposition(invocation,*input->borrowed_live_permit);
  if (!bound) owner->Fail(ERROR_INVALID_DATA);
  DispositionResult(owner, settled, *output); ReleaseSRWLockExclusive(&owner->gate);
  if (settled) delete owner;
}
static void RecoverOwned(const SsFinishedRecoverInputV1* input,const SsRecoveryCaptureOriginV2* origin,SsFinishedDispositionResultV1* output) {
  if (!output) return;
  *output = {}; output->state.abi_version = 1; output->state.status = Failed; output->state.primary_error = ERROR_INVALID_PARAMETER;
  output->state.primary_origin=FinishedCauseNative;
  output->source_state=SourceUncertain;output->metadata_state=MetadataUncertain;
  if (!input || input->abi_version != 1 || input->reserved || !input->borrowed_database || !input->borrowed_metrics ||
      !input->borrowed_live_recovery_permit || !ValidDeadline(input->original_current_operation)) return;
  auto owner=CreateLease(WindowsFiles(),input->current,input->original_current_operation,output->state,NativeLeaseAllocation);
  if(!owner)return;
  Invocation invocation(*owner,input->current);
  const auto& prepared=input->borrowed_live_recovery_permit->prepared;
  const bool settled = (!origin || BindRecoveryOrigin(invocation,*origin,prepared.subject,input->borrowed_live_recovery_permit->current_operation)) && Recover(invocation,*input);
  DispositionResult(owner, settled, *output);
  if (settled) delete owner;
}

void ss_finished_discard_v1(const SsFinishedDiscardInputV1* input,SsFinishedDispositionResultV1* output) {
  DiscardOwned(input,nullptr,output);
}
void ss_finished_discard_v2(const SsFinishedDiscardInputV2* input,SsFinishedDispositionResultV1* output) {
  if (!input || input->abi_version!=2 || input->reserved) {DiscardOwned(nullptr,nullptr,output);return;}
  DiscardOwned(&input->disposition,&input->origin,output);
}
void ss_finished_recover_disposition_v1(const SsFinishedRecoverInputV1* input,SsFinishedDispositionResultV1* output) {
  RecoverOwned(input,nullptr,output);
}
void ss_finished_recover_disposition_v2(const SsFinishedRecoverInputV2* input,SsFinishedDispositionResultV1* output) {
  if (!input || input->abi_version!=2 || input->reserved) {RecoverOwned(nullptr,nullptr,output);return;}
  RecoverOwned(&input->disposition,&input->origin,output);
}
