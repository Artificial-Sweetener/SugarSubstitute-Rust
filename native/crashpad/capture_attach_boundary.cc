// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_attach_boundary.h"

namespace sugarsubstitute {
namespace {
const CaptureBoundaryApi system_api = {
  &QueryPerformanceFrequency, &QueryPerformanceCounter, &GetLastError
};
}
CaptureAttachBoundary::CaptureAttachBoundary(OwnedAttach& owner,
    std::atomic<bool>& attempted, const AttachRuntime& runtime,
    const CaptureBoundaryApi* boundary)
    : owner_(owner), attempted_(attempted), runtime_(runtime),
      api_(boundary ? *boundary : system_api) {}

void CaptureAttachBoundary::Reject(SsCaptureAttachResultV2* output, DWORD error) {
  *output = {};
  output->abi_version = 2;
  output->status = 2;
  output->primary_error = error ? error : ERROR_GEN_FAILURE;
  output->primary_error_domain = static_cast<uint32_t>(NativeCauseDomain::Win32);
}
void CaptureAttachBoundary::Start(const SsCaptureAttachInputV2* input,
                                  SsCaptureAttachResultV2* output) {
  if (!output) return;
  Reject(output, ERROR_INVALID_PARAMETER);
  if (!input || !ValidAttachInput(*input)) return;
  LARGE_INTEGER frequency = {}, now = {};
  if (!api_.frequency(&frequency)) { Reject(output, api_.last_error()); return; }
  if (frequency.QuadPart <= 0 || frequency.QuadPart != input->counter_frequency) {
    Reject(output, ERROR_INVALID_DATA); return;
  }
  if (!api_.counter(&now)) { Reject(output, api_.last_error()); return; }
  if (now.QuadPart < 0) { Reject(output, ERROR_INVALID_DATA); return; }
  if (now.QuadPart >= input->attach_counter) { Reject(output, ERROR_TIMEOUT); return; }
  bool expected = false;
  if (!attempted_.compare_exchange_strong(expected, true)) {
    Reject(output, ERROR_ALREADY_INITIALIZED); return;
  }
  owner_.runtime = &runtime_;
  if (owner_.Begin(*input)) {
    const SsCaptureAttachProgressV2 progress = {
      2, sizeof(SsCaptureAttachProgressV2), reinterpret_cast<uintptr_t>(&owner_),
      input->cleanup, input->issue
    };
    owner_.Poll(progress);
  }
  owner_.Facts(output);
}
void CaptureAttachBoundary::Poll(const SsCaptureAttachProgressV2* progress,
                                 SsCaptureAttachResultV2* output) {
  if (!output) return;
  Reject(output, ERROR_INVALID_PARAMETER);
  if (!progress || !ValidAttachProgress(*progress) ||
      progress->attach != reinterpret_cast<uintptr_t>(&owner_)) return;
  owner_.Poll(*progress);
  owner_.Facts(output);
}
void CaptureAttachBoundary::Step(const SsCaptureAttachStepV2* step,
                                 SsCaptureAttachResultV2* output) {
  if (!output) return;
  Reject(output, ERROR_INVALID_PARAMETER);
  if (!step || !ValidAttachStep(*step) ||
      step->attach != reinterpret_cast<uintptr_t>(&owner_)) return;
  const bool accepted = owner_.Step(*step);
  owner_.Facts(output);
  if (!accepted) {
    // Local request rejection does not poison a different/expired native stage.
    // Exact owner's custody facts and earlier real causes still cross first.
    output->status = 2;
    if (!output->primary_error) {
      output->primary_error = ERROR_INVALID_STATE;
      output->primary_error_domain = static_cast<uint32_t>(NativeCauseDomain::Win32);
    }
  }
}
void CaptureAttachBoundary::Retire(const SsCaptureAttachProgressV2* progress,
                                   SsCaptureAttachResultV2* output) {
  if (!output) return;
  Reject(output, ERROR_INVALID_PARAMETER);
  if (!progress || !ValidAttachProgress(*progress) ||
      progress->attach != reinterpret_cast<uintptr_t>(&owner_)) return;
  owner_.Retire(*progress);
  owner_.Facts(output);
}
}  // namespace sugarsubstitute
