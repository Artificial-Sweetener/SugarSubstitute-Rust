// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_OWNED_ATTACH_H_
#define SUGARSUBSTITUTE_OWNED_ATTACH_H_
#include "sugarsubstitute/capture_attach_validation.h"
#include "sugarsubstitute/owned_connection.h"

namespace sugarsubstitute {
struct OwnedAttach;
// The external Crashpad runtime boundary. All three finite calls run outside
// owner locks on the original worker and adopt into this exact durable owner.
struct AttachRuntime {
  bool (*initialize)(OwnedAttach*);
  bool (*inspect_event)(OwnedAttach*, size_t);
  bool (*validate_events)(OwnedAttach*);
  bool (*install_handlers)(OwnedAttach*);
};
enum class AttachStage : uint32_t {
  Open = 1, Initialize = 2, Issue = 3, Validate = 4, Install = 5, Publish = 6, Done = 7
};
enum class ResponseEvent { Unknown, Owned, CallbackPinned };

// One process-pinned actual owner, with no retained callback/input pointers.
// Native handles in facts are borrowed from the Execution capsule through its
// checked retirement. The completion event is never closed by this owner.
struct OwnedAttach {
  OwnedConnection connection;
  HANDLE creator = nullptr;
  HANDLE handler = nullptr;
  HANDLE completion_event = nullptr;
  HANDLE accepted_pipe = INVALID_HANDLE_VALUE;
  DWORD creator_pid = 0, handler_pid = 0;
  uint64_t creator_birth = 0, handler_birth = 0;
  wchar_t endpoint[257] = {};
  char build[65] = {};
  uint8_t coordinator_namespace[32] = {}, coordinator_epoch[16] = {};
  uint64_t instance = 0, generation = 0;
  HANDLE response_events[3] = {};
  ResponseEvent response_custody[3] = {};
  const AttachRuntime* runtime = nullptr;
  AttachStage stage = AttachStage::Open;
  SsCaptureCleanupBudgetV1 cleanup = {};
  CleanupLimit cleanup_limit;
  DWORD worker_thread = 0;
  uint64_t nonce = 0;
  DWORD response_bytes = 0;
  bool started = false, stopped = false, expired = false;
  bool issuing = false, issued = false, response_complete = false;
  bool response_recorded = false;
  bool globals_committed = false, handlers_started = false, registered = false;
  bool retired = false;

  bool Begin(const SsCaptureAttachInputV2& input);
  // At most one accepted finite stage or one zero-wait completion per call.
  void Poll(const SsCaptureAttachProgressV2& progress);
  // Called only while the exact per-call callback token is active. Finite stages
  // merely claim here; Issue alone performs overlapped native acceptance.
  bool Step(const SsCaptureAttachStepV2& step);
  void Retire(const SsCaptureAttachProgressV2& progress);
  void Facts(SsCaptureAttachResultV2* output);
  bool ValidateProcesses();
  bool Open();
  bool Issue();
  bool Complete();
  void RecordResponse();
  bool AdoptResponse();
  bool InspectEvents(CleanupLimit* limit);
  bool CloseEvents();
  bool LatchCleanup(const SsCaptureCleanupSourceV1& source);
 private:
  bool claim_used_ = false;
  bool call_active_ = false;
  bool Accept(AttachStage accepted);
};
}  // namespace sugarsubstitute
#endif
