// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_ATTACH_BOUNDARY_H_
#define SUGARSUBSTITUTE_CAPTURE_ATTACH_BOUNDARY_H_
#include "sugarsubstitute/owned_attach.h"
#include <atomic>

namespace sugarsubstitute {
// Only the actual external native frequency/counter observations are replaceable.
struct CaptureBoundaryApi {
  decltype(&QueryPerformanceFrequency) frequency;
  decltype(&QueryPerformanceCounter) counter;
  decltype(&GetLastError) last_error;
};
// Same process-lifetime owner and role admission as parent capture. No allocation,
// thread, scheduling, borrowed callback retention or caller-token dereference.
class CaptureAttachBoundary {
 public:
  CaptureAttachBoundary(OwnedAttach& owner, std::atomic<bool>& attempted,
                        const AttachRuntime& runtime,
                        const CaptureBoundaryApi* boundary = nullptr);
  void Start(const SsCaptureAttachInputV2* input, SsCaptureAttachResultV2* output);
  void Poll(const SsCaptureAttachProgressV2* progress, SsCaptureAttachResultV2* output);
  // Pure fixed token validation plus exact owner's nested acceptance. No native
  // query/provider occurs here before the owner's nonblocking Issue operation.
  void Step(const SsCaptureAttachStepV2* step, SsCaptureAttachResultV2* output);
  void Retire(const SsCaptureAttachProgressV2* progress, SsCaptureAttachResultV2* output);
 private:
  OwnedAttach& owner_;
  std::atomic<bool>& attempted_;
  const AttachRuntime& runtime_;
  const CaptureBoundaryApi& api_;
  static void Reject(SsCaptureAttachResultV2* output, DWORD error);
};
}  // namespace sugarsubstitute
#endif
