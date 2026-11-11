// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_BRIDGE_H_
#define SUGARSUBSTITUTE_CAPTURE_BRIDGE_H_
#include <stdint.h>
#include <stddef.h>

// Each array is borrowed for the synchronous call only. Paths are bounded UTF-16
// without embedded NUL; annotations are fixed bounded ASCII, not credentials.
struct SsCaptureInput {
  const uint16_t* handler;
  size_t handler_length;
  const uint16_t* database;
  size_t database_length;
  const uint16_t* metrics;
  size_t metrics_length;
  const char* build;
  size_t build_length;
  const char* coordinator;
  size_t coordinator_length;
  int64_t deadline_counter;
  int64_t counter_frequency;
};

struct SsCaptureResult {
  uint32_t abi_version;
  uint32_t registered;
  uint32_t error;
  uint32_t cleanup_error;
  uint32_t pending_io;
  uint32_t retired;
  uintptr_t process;
  uintptr_t thread;
  uint32_t cleanup_count;
  uint32_t cleanup_codes[8];
  uint64_t omitted_cleanup;
};

// Outputs transfer once even on failure. The caller adopts nonzero handles
// before examining registered/error. Client callbacks and module are pinned
// until process exit; these functions never provide an unload capability.
extern "C" __declspec(dllexport) void ss_capture_start_v1(
    const SsCaptureInput* input, SsCaptureResult* result);
// Finite zero-wait completion/checked-close attempt; failure retains actual I/O.
extern "C" __declspec(dllexport) void ss_capture_retire_io_v1(
    SsCaptureResult* result);
#endif
