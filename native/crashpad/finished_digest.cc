// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_digest.h"
#include <winternl.h>
namespace sugarsubstitute::finished {
namespace {
DWORD Error(NTSTATUS status) noexcept {
  return status >= 0 ? ERROR_SUCCESS : RtlNtStatusToDosError(status);
}
}
DWORD BeginHash(HashState& state, ParseControl& control) noexcept {
  if (state.algorithm || state.hash) return ERROR_INVALID_STATE;
  // The API writes directly into retained slots before its result is read.
  const auto before = control.Check();
  if (before.interruption != Interruption::None) return before.cause;
  auto status = BCryptOpenAlgorithmProvider(&state.algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0);
  if (status < 0) return Error(status);
  ULONG bytes = 0, returned = 0;
  const auto property = control.Check();
  if (property.interruption != Interruption::None) return property.cause;
  status = BCryptGetProperty(state.algorithm, BCRYPT_OBJECT_LENGTH,
      reinterpret_cast<PUCHAR>(&bytes), sizeof(bytes), &returned, 0);
  if (status < 0) return Error(status);
  if (returned != sizeof(bytes) || bytes == 0 || bytes > state.object.size())
    return ERROR_INVALID_DATA;
  const auto create = control.Check();
  if (create.interruption != Interruption::None) return create.cause;
  return Error(BCryptCreateHash(state.algorithm, &state.hash,
      state.object.data(), bytes, nullptr, 0, 0));
}
DWORD UpdateHash(HashState& state, std::span<const uint8_t> bytes) noexcept {
  if (!state.hash || bytes.size() > 65536) return ERROR_INVALID_PARAMETER;
  return Error(BCryptHashData(state.hash, const_cast<PUCHAR>(bytes.data()),
      static_cast<ULONG>(bytes.size()), 0));
}
DWORD FinishHash(HashState& state, std::span<uint8_t, 32> digest) noexcept {
  if (!state.hash) return ERROR_INVALID_STATE;
  return Error(BCryptFinishHash(state.hash, digest.data(), 32, 0));
}
DWORD RetireHashPart(HashState& state, bool hash) noexcept {
  if (hash && state.hash) {
    const auto error = Error(BCryptDestroyHash(state.hash));
    if (error) return error;
    state.hash = nullptr;
  }
  if (!hash && state.algorithm) {
    if (state.hash) return ERROR_INVALID_STATE;
    const auto error = Error(BCryptCloseAlgorithmProvider(state.algorithm, 0));
    if (error) return error;
    state.algorithm = nullptr;
  }
  return ERROR_SUCCESS;
}
DWORD RetireHash(HashState& state) noexcept {
  const auto first = RetireHashPart(state, true);
  return first ? first : RetireHashPart(state, false);
}
}  // namespace sugarsubstitute::finished
