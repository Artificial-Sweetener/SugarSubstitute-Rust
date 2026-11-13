// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_FINISHED_REPORT_H_
#define SUGARSUBSTITUTE_FINISHED_REPORT_H_
#include "finished_io.h"
#include "incident_bridge.h"
#include <algorithm>

namespace sugarsubstitute::finished {
enum Slot : size_t { Database, Metrics, Reports, Metadata, Source, Candidate,
                    OriginalStage, CandidateStage, ScanDirectory, SlotCount };
// One owner holds every partial native output and bounded image. Its destructor
// performs no native cleanup. Only checked retirement can destroy the owner.
struct QueryIdentity {
  uintptr_t borrowed_database = 0, borrowed_metrics = 0;
  uint8_t expected_database_identity[24] = {}, expected_metrics_identity[24] = {};
  SsFinishedSubjectV1 subject{};
  uint32_t build_length = 0;
  char build[64] = {};
  uint8_t optional_uuid[16] = {};
  uint32_t has_uuid = 0;
};
class Lease final {
 public:
  explicit Lease(FileApi& api) noexcept : api(api) {}
  FileApi& api;
  SRWLOCK gate = SRWLOCK_INIT;
  std::array<HANDLE, SlotCount> handles{};
  HashState hash;
  SsFinishedDeadlineV1 operation{}, cleanup{};
  int64_t last_counter = 0;
  bool cleanup_latched = false, in_cleanup = false, locked = false;
  bool queried = false, prepared = false, facts_valid = false, disposed = false;
  bool recovering = false, source_absent = false;
  SsFinishedStatusV1 status = Failed;
  DWORD primary = 0;
  SsFinishedCauseOriginV1 primary_origin = FinishedCauseNone;
  uint32_t cleanup_origin_bits = 0;
  std::array<DWORD, 8> secondary{};
  uint32_t secondary_count = 0;
  uint64_t omitted = 0, read_bytes = 0, reclaimed = 0;
  uint32_t absent_mask = 0;
  SsCaptureSourceStateV1 source_state = SourceRetained;
  SsCaptureMetadataStateV1 metadata_state = OriginalImage;
  SsFinishedFactsV1 facts{};
  SsCapturePreparationV1 preparation{};
  SsCaptureDispositionV1 disposition{};
  QueryIdentity expected{};
  SsRecoveryCaptureOriginV2 recovery_origin{};
  SsLogicalStoreOperationV1 recovery_current_operation{};
  bool recovery_origin_bound = false;
  std::array<uint8_t, kMetadataLimit> metadata{}, original{}, candidate{};
  // FILE_ID_BOTH_DIR_INFO pages require eight-byte alignment, including this
  // member after byte-sized recovery state and metadata images.
  alignas(FILE_ID_BOTH_DIR_INFO) std::array<uint8_t, 65536> scratch{};
  uint32_t metadata_bytes = 0, candidate_bytes = 0;
  std::array<wchar_t, 41> source_name{};
  std::array<std::array<wchar_t, 80>, 2> stage_names{};
  uint64_t read_size = 0;
  size_t read_slot = Source;
  bool Fail(DWORD error, SsFinishedCauseOriginV1 origin=FinishedCauseNative) noexcept;
  void CleanupError(DWORD error, SsFinishedCauseOriginV1 origin=FinishedCauseNative) noexcept;
  void Result(SsFinishedResultV1& output) noexcept;
};
// This stack-only invocation owns the ephemeral current-control borrow. No
// callback/state pointer is present in Lease or any returned native token.
class Invocation final : public ParseControl, public DumpSource {
 public:
  Invocation(Lease& state, const SsFinishedControlSourceV1& current) noexcept : state(state), control_source(current) {}
  Lease& state;
  const SsFinishedControlSourceV1 control_source;
  Checkpoint echoed_stop{};
  bool Allowed() noexcept;
  bool Post(DWORD error) noexcept;
  Checkpoint Check() noexcept override;
  ReadResult Read(uint64_t offset, std::span<uint8_t> output) noexcept override;
  bool Open(Slot slot, Slot parent, const wchar_t* leaf, OpenMode mode) noexcept;
  bool Close(Slot slot) noexcept;
  bool Info(Slot slot, Slot parent, FileInfo& output) noexcept;
  bool RetireDigest() noexcept;
  bool Digest(Slot slot, uint64_t bytes, uint8_t* output) noexcept;
  bool DigestBytes(std::span<const uint8_t> bytes, uint8_t* output) noexcept;
  bool Image(Slot slot, std::span<uint8_t> image, uint64_t bytes) noexcept;
  bool FileFacts(Slot slot, Slot parent, uint64_t limit, SsCaptureFileFactsV1& output) noexcept;
  bool LockMetadata(bool exclusive) noexcept;
  bool UnlockMetadata() noexcept;
  bool Absent(Slot parent, const wchar_t* leaf) noexcept;
  bool Retire() noexcept;
};
// Validate the complete typed relation through the same real hash and current-control owner.
bool RecoverySubject(const SsRecoveryCaptureOriginV2&, const SsFinishedSubjectV1&) noexcept;
bool RecoveryOperation(const SsRecoveryCaptureOriginV2&, const SsLogicalStoreOperationV1&) noexcept;
bool BindRecoveryOrigin(Invocation&, const SsRecoveryCaptureOriginV2&, const SsFinishedSubjectV1&,
                        const SsLogicalStoreOperationV1&) noexcept;
void OpenOwned(const SsFinishedOpenInputV1&,const SsRecoveryCaptureOriginV2*,
               const SsLogicalStoreOperationV1*,SsFinishedResultV1&) noexcept;
static_assert(sizeof(Lease) + SsFinishedTemporaryBytesV1 <= SsFinishedOwnerStorageBytesV1);
static_assert(static_cast<uint32_t>(SlotCount) + 2 <= SsFinishedNativeOutputsV1);
Checkpoint ObserveCurrent(const SsFinishedControlSourceV1& source) noexcept;
Checkpoint ObserveCounter(FileApi& api, const SsFinishedDeadlineV1& cutoff, int64_t& last, int64_t frequency) noexcept;
bool Preflight(FileApi& api, const SsFinishedControlSourceV1& source, const SsFinishedDeadlineV1& cutoff, SsFinishedResultV1& output) noexcept;
// The only substituted allocation boundary creates this exact concrete owner.
// No arbitrary byte demand, layout or allocation authority enters the factory.
using AllocateLease = Lease* (*)(FileApi& api) noexcept;
Lease* NativeLeaseAllocation(FileApi& api) noexcept;
Lease* CreateLease(FileApi& api, const SsFinishedControlSourceV1& source,
                   const SsFinishedDeadlineV1& cutoff, SsFinishedResultV1& output,
                   AllocateLease allocate) noexcept;
void BusyResult(SsFinishedResultV1& output, uintptr_t lease) noexcept;
SsFinishedStatusV1 Status(ParseStatus status) noexcept;
bool ValidDeadline(const SsFinishedDeadlineV1& value) noexcept;
bool ValidSubject(const SsFinishedSubjectV1& value) noexcept;
bool ValidBuild(uint32_t length, const char* build) noexcept;
bool SameOperation(const SsLogicalStoreOperationV1& a,
                   const SsLogicalStoreOperationV1& b) noexcept;
bool ValidOperation(const SsLogicalStoreOperationV1& value) noexcept;
bool SameFacts(const SsCaptureFileFactsV1& a,
               const SsCaptureFileFactsV1& b) noexcept;
// Verify the canonical producer-created structural directory through the pinned
// database using the existing scan slot and original control/cutoff. Unsupported
// contents and error-returned handles remain owned for checked retirement.
bool InventoryAttachments(Invocation& invocation) noexcept;
bool Query(Invocation& invocation) noexcept;
bool Prepare(Invocation& invocation, const SsLogicalStoreOperationV1& operation) noexcept;
bool Disposition(Invocation& invocation, const SsCaptureDispositionV1& permit) noexcept;
bool Recover(Invocation& invocation, const SsFinishedRecoverInputV1& input) noexcept;
bool StageNames(Invocation& invocation, const SsLogicalStoreOperationV1& key) noexcept;
}  // namespace sugarsubstitute::finished
#endif
