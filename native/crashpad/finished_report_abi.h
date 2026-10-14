// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_FINISHED_REPORT_ABI_H_
#define SUGARSUBSTITUTE_FINISHED_REPORT_ABI_H_
#include <stdint.h>
#include <stddef.h>

// Closed producer envelopes, funded by the real caller storage owner before FFI.
// These facts do not confer allocation credit or native/source authority.
constexpr uint64_t SsFinishedOwnerStorageBytesV1 = 4ull * 1024 * 1024;
constexpr uint64_t SsFinishedTemporaryBytesV1 = 512ull * 1024;
constexpr uint32_t SsFinishedNativeOutputsV1 = 11;
// Windows x64 ABI1, default eight-byte packing. All reserved fields must be zero.
// Caller must provide aligned complete structs. Output is initialized before validation.
// uintptr fields are actual opaque native leases or borrowed handles, never proof data.
enum SsFinishedStatusV1 : uint32_t {
  Finished=0, Missing=1, Busy=2, Corrupt=3, WrongIdentity=4,
  Oversized=5, DeadlineElapsed=6, Failed=7
};
enum SsFinishedSubjectTagV1 : uint32_t { Desktop=1, Supervisor=2 };
enum SsFinishedPhaseV1 : uint32_t { Operation=1, Cleanup=2 };
struct SsFinishedSubjectV1 {
  uint32_t tag;
  uint8_t namespace_id[32];
  uint8_t epoch[16];
  uint32_t reserved;
  uint64_t instance;
  uint64_t generation;
};
// Subject struct is72 bytes. Supervisor instance/generation=0; Desktop both>0.
// UUID bytes use canonical big-endian RFC form, converted from native metadata fields.
struct SsFinishedDeadlineV1 {
  int64_t counter_deadline;
  int64_t counter_frequency;
};
struct SsFinishedFactsV1 {
  SsFinishedSubjectV1 subject;
  uint8_t uuid[16];
  uint32_t build_length;
  char build[64];
  uint32_t reserved;
  uint64_t source_bytes;
  uint8_t source_file_identity[24];
  uint64_t observed_temporary_bytes;
  uint32_t observed_entries;
  uint32_t inventory_complete;
};
enum SsFinishedCauseOriginV1 : uint32_t {
  FinishedCauseNone=0, FinishedCauseNative=1,
  FinishedCauseControl=2, FinishedCauseQpc=3
};
struct SsFinishedResultV1 {
  uint32_t abi_version;
  SsFinishedStatusV1 status;
  uint32_t primary_error;
  uint32_t cleanup_count;
  uint32_t cleanup_codes[8];
  uint64_t omitted_cleanup;
  uintptr_t lease;
  // Actual zero-native-owner disposition: no Lease created, or checked complete
  // retirement. It never proves source/stage absence or successful disposition.
  uint32_t retired;
  uint32_t live_handles;
  uint32_t live_locks;
  SsFinishedCauseOriginV1 primary_origin;
  uint32_t facts_valid;
  uint32_t cleanup_origin_bits; // Eight two-bit origins; unused/high bits zero.
  SsFinishedFactsV1 facts;
  uint64_t actual_read_bytes;
  uint64_t actual_reclaimed_bytes;
};
enum SsFinishedControlStatusV1 : uint32_t {
  FinishedControlCurrent=0, FinishedControlCancelled=1,
  FinishedControlTimedOut=2, FinishedControlFailed=3
};
// A synchronous borrowed current-context observation, never native authority.
// The state and noexcept callback stay live for this invocation only. Native
// tokens do not store either pointer. Rust catches unwind and retains causes.
struct SsFinishedControlSourceV1 {
  uintptr_t borrowed_state;
  SsFinishedControlStatusV1 (__cdecl* check)(uintptr_t, uint32_t* cause) noexcept;
};
struct SsFinishedOpenInputV1 {
  uint32_t abi_version;
  uint32_t reserved;
  uintptr_t borrowed_database;
  uintptr_t borrowed_metrics;
  uint8_t expected_database_identity[24];
  uint8_t expected_metrics_identity[24];
  SsFinishedSubjectV1 subject;
  uint32_t build_length;
  char build[64];
  uint8_t optional_uuid[16];
  uint32_t has_uuid;
  SsFinishedDeadlineV1 original_operation;
  SsFinishedControlSourceV1 current;
};
struct SsFinishedCallV1 {
  uint32_t abi_version;
  SsFinishedPhaseV1 phase;
  uintptr_t lease;
  SsFinishedDeadlineV1 original;
  SsFinishedControlSourceV1 current;
};
struct SsFinishedReadInputV1 {
  SsFinishedCallV1 call;
  uint64_t offset;
  uint8_t* output;
  uint32_t capacity;
  uint32_t reserved;
};
struct SsLogicalStoreOperationV1 {
  uint8_t namespace_id[32];
  uint8_t epoch[16];
  uint64_t id;
};
struct SsCaptureFileFactsV1 {
  uint8_t identity[24];
  uint64_t bytes;
  uint8_t sha256[32];
};
enum SsCaptureStageRoleV1 : uint32_t { OriginalMetadata=1, CandidateMetadata=2 };
struct SsCaptureStageFactsV1 {
  SsCaptureStageRoleV1 role;
  uint32_t reserved;
  SsCaptureFileFactsV1 file;
};
struct SsCapturePreparationV1 {
  uint32_t abi_version;
  uint32_t reserved;
  SsLogicalStoreOperationV1 historical_operation;
  SsFinishedSubjectV1 subject;
  uint8_t uuid[16];
  uint32_t build_length;
  char build[64];
  uint32_t reserved_build;
  uint8_t database_identity[24];
  uint8_t metrics_identity[24];
  SsCaptureFileFactsV1 metadata_preimage;
  SsCaptureFileFactsV1 source;
  SsCaptureStageFactsV1 original_stage;
  SsCaptureStageFactsV1 candidate_stage;
};
struct SsFinishedPrepareInputV1 {
  SsFinishedCallV1 call;
  SsLogicalStoreOperationV1 operation;
};
struct SsFinishedPrepareResultV1 {
  SsFinishedResultV1 state;
  uint32_t preparation_valid;
  uint32_t reserved;
  SsCapturePreparationV1 preparation;
};
enum SsCaptureAuthorizationV1 : uint32_t { NormalDisposition=1, RecoveryDisposition=2 };
// Private native lowering consumes a live opaque repository proof. Neither this
// struct nor public matching facts are accepted as a proof at a Rust/IPC boundary.
struct SsCaptureDispositionV1 {
  SsCapturePreparationV1 prepared;
  uint32_t disposition; //1 exact promoted native payload;2 exact explicit discard.
  uint32_t discard_reason; //0 promotion;1 AcknowledgedRetention,2 StoragePressure,3 Oversized,4 UserRequested.
  uint64_t incident_revision;
  SsCaptureFileFactsV1 committed_summary;
  uint32_t has_payload;
  uint32_t reserved_payload;
  SsCaptureFileFactsV1 committed_payload; //all zero only for explicit discard.
  SsCaptureFileFactsV1 committed_intent;
  uint8_t canonical_plan_sha256[32];
  SsLogicalStoreOperationV1 current_operation;
  SsCaptureAuthorizationV1 authorization;
  uint32_t reserved_authorization;
};
// Existing discarded-read call gets a normal permit only; recovery cannot call it.
struct SsFinishedDiscardInputV1 {
  SsFinishedCallV1 call;
  const SsCaptureDispositionV1* borrowed_live_permit;
};
struct SsFinishedRecoverInputV1 {
  uint32_t abi_version;
  uint32_t reserved;
  uintptr_t borrowed_database;
  uintptr_t borrowed_metrics;
  uint8_t expected_database_identity[24];
  uint8_t expected_metrics_identity[24];
  const SsCaptureDispositionV1* borrowed_live_recovery_permit;
  SsFinishedDeadlineV1 original_current_operation;
  SsFinishedControlSourceV1 current;
};
enum SsCaptureSourceStateV1 : uint32_t {
  SourceRetained=1, DeletedNow=2, PriorDeletionReconciled=3, SourceUncertain=4
};
enum SsCaptureMetadataStateV1 : uint32_t {
  OriginalImage=1, CandidateImage=2, MetadataUncertain=3
};
struct SsFinishedDispositionResultV1 {
  SsFinishedResultV1 state;
  SsCaptureSourceStateV1 source_state;
  SsCaptureMetadataStateV1 metadata_state;
  uint32_t stages_absent_mask; //bit0 original,bit1 candidate, each actual checked absence.
  uint32_t reserved;
};
extern "C" __declspec(dllexport) void ss_finished_prepare_disposition_v1(
  const SsFinishedPrepareInputV1* input, SsFinishedPrepareResultV1* output);
extern "C" __declspec(dllexport) void ss_finished_discard_v1(
  const SsFinishedDiscardInputV1* input, SsFinishedDispositionResultV1* output);
extern "C" __declspec(dllexport) void ss_finished_recover_disposition_v1(
  const SsFinishedRecoverInputV1* input, SsFinishedDispositionResultV1* output);
// Six exports: open/read/retire and prepare/discard/recover.
// All output.native state is initialized and actual partial token adopted first.
// Prepare has no metadata/source mutation; OriginalMetadata/CandidateMetadata are
// actual protected CREATE_NEW files, fixed derived names, <=1MiB each, checked
// flush/readback and exact file facts retained before preparation_valid=1.
// NormalDiscard requires NormalDisposition, current==historical and same live
// native prepared state. Recover requires RecoveryDisposition and matching sealed
// NEW distinct current operation plus exact historical facts; source may actually be absent.
// Recovery does not recreate a historical permit, ordinary source lease or stages.
// It never uses prior elapsed moments as current authority or samples a new budget.
// No stages_absent bit or actual_reclaimed_bytes on delete-pending/failed close.
// PriorDeletionReconciled always reports zero newly reclaimed bytes.
static_assert(sizeof(SsLogicalStoreOperationV1)==56);
static_assert(sizeof(SsCaptureFileFactsV1)==64);
static_assert(sizeof(SsCaptureStageFactsV1)==72);
static_assert(sizeof(SsCapturePreparationV1)==544);
static_assert(sizeof(SsFinishedPrepareInputV1)==104);
static_assert(sizeof(SsFinishedPrepareResultV1)==864);
static_assert(sizeof(SsCaptureDispositionV1)==856);
static_assert(sizeof(SsFinishedRecoverInputV1)==112);
static_assert(sizeof(SsFinishedDispositionResultV1)==328);
extern "C" __declspec(dllexport) void ss_finished_open_v1(const SsFinishedOpenInputV1*, SsFinishedResultV1*);
extern "C" __declspec(dllexport) void ss_finished_read_v1(const SsFinishedReadInputV1*, SsFinishedResultV1*);
extern "C" __declspec(dllexport) void ss_finished_retire_v1(const SsFinishedCallV1*, SsFinishedResultV1*);

static_assert(sizeof(SsFinishedSubjectV1)==72);
static_assert(sizeof(SsFinishedFactsV1)==208);
static_assert(sizeof(SsFinishedResultV1)==312);
static_assert(sizeof(SsFinishedOpenInputV1)==264);
static_assert(sizeof(SsFinishedCallV1)==48);
static_assert(sizeof(SsFinishedReadInputV1)==72);
static_assert(sizeof(SsFinishedDiscardInputV1)==56);
static_assert(sizeof(SsFinishedControlSourceV1)==16);
static_assert(offsetof(SsFinishedResultV1,primary_origin)==76);
static_assert(offsetof(SsFinishedResultV1,cleanup_origin_bits)==84);
static_assert(offsetof(SsFinishedCallV1,current)==32);
static_assert(offsetof(SsFinishedOpenInputV1,current)==248);
static_assert(offsetof(SsFinishedRecoverInputV1,current)==96);
#endif
