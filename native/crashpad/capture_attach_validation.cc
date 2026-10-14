// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_attach_validation.h"
#include <limits>

namespace sugarsubstitute {
namespace {
bool Handle(uintptr_t value) {
  return value != 0 && value < std::numeric_limits<uintptr_t>::max() - 15;
}
bool Callbacks(const SsCaptureCleanupSourceV1& cleanup,
               const SsCaptureIssueSourceV2& issue) {
  return cleanup.borrowed_context && cleanup.request &&
         issue.borrowed_context && issue.request;
}
template <typename T, size_t N>
bool ZeroTail(const T (&value)[N], size_t length) {
  for (size_t i = length; i < N; ++i) if (value[i]) return false;
  return true;
}
bool Build(const SsCaptureAttachInputV2& input) {
  if (!input.build_length || input.build_length > 64) return false;
  for (size_t i = 0; i < input.build_length; ++i) {
    const char c = input.build[i];
    if (!(c >= '0' && c <= '9') && !(c >= 'a' && c <= 'z') &&
        !(c >= 'A' && c <= 'Z') && c != '-' && c != '_') return false;
  }
  return ZeroTail(input.build, input.build_length);
}
bool Endpoint(const SsCaptureAttachInputV2& input) {
  const uint16_t prefix[] = {'\\', '\\', '.', '\\', 'p', 'i', 'p', 'e', '\\'};
  if (input.endpoint_length <= 9 || input.endpoint_length > 256) return false;
  for (size_t i = 0; i < 9; ++i) if (input.endpoint[i] != prefix[i]) return false;
  for (size_t i = 9; i < input.endpoint_length; ++i) if (!input.endpoint[i]) return false;
  return ZeroTail(input.endpoint, input.endpoint_length);
}
}  // namespace

bool ValidAttachInput(const SsCaptureAttachInputV2& input) {
  bool epoch = false;
  for (uint8_t value : input.coordinator_epoch) epoch |= value != 0;
  return input.abi_version == 2 && input.byte_size == sizeof(input) &&
         ((input.subject == 1 && input.instance && input.generation) ||
          (input.subject == 2 && input.instance == 0 && input.generation == 0)) &&
         input.provenance == 1 && epoch && input.creator_pid && input.handler_pid &&
         input.creator_pid != input.handler_pid && input.creator_birth && input.handler_birth &&
         Handle(input.creator_process) && Handle(input.handler_process) &&
         input.creator_process != input.handler_process &&
         input.attach_counter > 0 && input.counter_frequency > 0 &&
         !input.reserved_deadline0 && !input.reserved_deadline1 &&
         Build(input) && Endpoint(input) && Callbacks(input.cleanup, input.issue) &&
         Handle(input.borrowed_completion_event) &&
         input.borrowed_completion_event != input.creator_process &&
         input.borrowed_completion_event != input.handler_process;
}
bool ValidAttachProgress(const SsCaptureAttachProgressV2& progress) {
  return progress.abi_version == 2 && progress.byte_size == sizeof(progress) &&
         Handle(progress.attach) && Callbacks(progress.cleanup, progress.issue);
}
bool ValidAttachStep(const SsCaptureAttachStepV2& step) {
  return step.abi_version == 2 && step.byte_size == sizeof(step) &&
         Handle(step.attach) && step.stage >= 1 && step.stage <= 6 &&
         !step.reserved && step.nonce;
}
}  // namespace sugarsubstitute
