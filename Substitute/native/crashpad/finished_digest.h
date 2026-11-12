// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_FINISHED_DIGEST_H_
#define SUGARSUBSTITUTE_FINISHED_DIGEST_H_
#include <windows.h>
#include "bounded_metadata.h"
#include <bcrypt.h>
#include <array>
#include <span>
namespace sugarsubstitute::finished {
// Both CNG outputs and their fixed caller-owned object storage survive failure.
// No destructor destroys native state; explicit checked retirement does so.
struct HashState {
  BCRYPT_ALG_HANDLE algorithm = nullptr;
  BCRYPT_HASH_HANDLE hash = nullptr;
  std::array<UCHAR, 8192> object{};
};
DWORD BeginHash(HashState& state, ParseControl& control) noexcept;
DWORD UpdateHash(HashState& state, std::span<const uint8_t> bytes) noexcept;
DWORD FinishHash(HashState& state, std::span<uint8_t, 32> digest) noexcept;
DWORD RetireHashPart(HashState& state, bool hash) noexcept;
DWORD RetireHash(HashState& state) noexcept;
}  // namespace sugarsubstitute::finished
#endif
