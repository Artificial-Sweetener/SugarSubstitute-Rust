// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_ENDPOINT_H_
#define SUGARSUBSTITUTE_CAPTURE_ENDPOINT_H_
#include <stdint.h>
#include <stddef.h>

struct SsCaptureEndpointExpectationV1 {
  uint32_t abi_version, creator_pid;
  uint64_t creator_birth;
  uint32_t handler_pid, reserved;
  uint64_t handler_birth;
};
struct SsCaptureEndpointV1 {
  uint32_t abi_version, status, provenance, length;
  uint32_t creator_pid, handler_pid;
  uint64_t creator_birth, handler_birth;
  uint32_t error, reserved;
  uint16_t endpoint[256];
};
// Inert source matching facts; these getters grant no native capability.
struct SsCaptureSourceIdentityV1 {
  uint32_t abi_version, status, creator_pid, handler_pid;
  uint64_t creator_birth, handler_birth;
  uint32_t reserved[2];
};
static_assert(sizeof(SsCaptureEndpointExpectationV1) == 32);
static_assert(sizeof(SsCaptureEndpointV1) == 560);
static_assert(offsetof(SsCaptureEndpointV1, endpoint) == 48);
static_assert(sizeof(SsCaptureSourceIdentityV1) == 40);
extern "C" __declspec(dllexport) bool ss_capture_endpoint_v1(
    const SsCaptureEndpointExpectationV1*, SsCaptureEndpointV1*);
extern "C" __declspec(dllexport) bool ss_capture_source_identity_v1(
    SsCaptureSourceIdentityV1*);
#endif
