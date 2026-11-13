// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_source.h"

namespace sugarsubstitute {
bool CaptureSource::Publish(const OwnedStart& owner, const std::wstring& endpoint) {
  if (frozen_ || !owner.completed || owner.connection.pending ||
      owner.connection.pipe != INVALID_HANDLE_VALUE || owner.connection.event ||
      owner.initial_pipe || owner.bootstrap_process || !owner.creator_pid ||
      !owner.handler_pid || owner.creator_pid == owner.handler_pid ||
      !owner.creator_birth || !owner.handler_birth || endpoint.size() < 10 ||
      endpoint.size() >= 256 || endpoint.compare(0, 9, L"\\\\.\\pipe\\") != 0)
    return false;
  for (const auto unit : endpoint) if (!unit) return false;
  endpoint_.abi_version = 1;
  endpoint_.status = 1;
  endpoint_.provenance = 1;
  endpoint_.length = static_cast<uint32_t>(endpoint.size());
  endpoint_.creator_pid = owner.creator_pid;
  endpoint_.handler_pid = owner.handler_pid;
  endpoint_.creator_birth = owner.creator_birth;
  endpoint_.handler_birth = owner.handler_birth;
  for (size_t i = 0; i < endpoint.size(); ++i)
    endpoint_.endpoint[i] = static_cast<uint16_t>(endpoint[i]);
  identity_ = {1, 1, owner.creator_pid, owner.handler_pid,
               owner.creator_birth, owner.handler_birth, {0, 0}};
  frozen_ = true;
  available_.store(true, std::memory_order_release);
  return true;
}
void CaptureSource::Revoke() { available_.store(false, std::memory_order_release); }
bool CaptureSource::Endpoint(const SsCaptureEndpointExpectationV1* expected,
                             SsCaptureEndpointV1* output) const {
  if (!output) return false;
  *output = {};
  output->abi_version = 1;
  output->error = ERROR_INVALID_PARAMETER;
  if (!expected || expected->abi_version != 1 || expected->reserved) return false;
  if (!available_.load(std::memory_order_acquire)) {
    output->error = ERROR_INVALID_STATE;
    return false;
  }
  if (expected->creator_pid != identity_.creator_pid ||
      expected->handler_pid != identity_.handler_pid ||
      expected->creator_birth != identity_.creator_birth ||
      expected->handler_birth != identity_.handler_birth) {
    output->error = ERROR_ACCESS_DENIED;
    return false;
  }
  *output = endpoint_;
  return true;
}
bool CaptureSource::Identity(SsCaptureSourceIdentityV1* output) const {
  if (!output) return false;
  *output = {};
  output->abi_version = 1;
  if (!available_.load(std::memory_order_acquire)) return false;
  *output = identity_;
  return true;
}
}  // namespace sugarsubstitute
