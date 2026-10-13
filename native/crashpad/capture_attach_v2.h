// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_ATTACH_V2_H_
#define SUGARSUBSTITUTE_CAPTURE_ATTACH_V2_H_
#include "sugarsubstitute/capture_endpoint.h"
#include <stddef.h>
#include <stdint.h>

struct SsCaptureCleanupBudgetV1 {
  uint32_t abi_version, byte_size, state, reserved;
  int64_t start_counter, grace_counter, total_counter, counter_frequency;
};
struct SsCaptureCleanupSourceV1 {
  uintptr_t borrowed_context;
  bool (__cdecl* request)(uintptr_t, SsCaptureCleanupBudgetV1*);
};
// The synchronous callback observes user time outside every Rust/native lock.
// It claims exactly one finite stage, or issues one nonblocking native operation.
// Decision: 1 accepted, 2 stopped, 3 expired, 4 failed (original cause retained).
struct SsCaptureIssueSourceV2 {
  uintptr_t borrowed_context;
  uint32_t (__cdecl* request)(uintptr_t, uintptr_t, uint32_t, uint64_t);
};
struct SsCaptureAttachInputV2 {
  uint32_t abi_version, byte_size, subject, provenance;
  uint8_t coordinator_namespace[32], coordinator_epoch[16];
  uint64_t instance, generation;
  uint32_t creator_pid, handler_pid;
  uint64_t creator_birth, handler_birth;
  uintptr_t creator_process, handler_process;
  int64_t attach_counter, counter_frequency;
  int64_t reserved_deadline0, reserved_deadline1;
  uint32_t build_length, endpoint_length;
  char build[64];
  uint16_t endpoint[256];
  SsCaptureCleanupSourceV1 cleanup;
  SsCaptureIssueSourceV2 issue;
  uintptr_t borrowed_completion_event;
};
// Cause domains: 0 unused, 1 Win32, 2 NTSTATUS, 3 CRT errno. The cleanup domain word stores
// two bits per used slot; unused pairs and the upper16 bits are zero.
// Status: 0 unavailable, 1 registered, 2 failed, 3 retiring, 4 retired,
// 5 attaching. Registered is an actual installed-handler fact, not eligibility.
struct SsCaptureAttachResultV2 {
  uint32_t abi_version, status, registered, pending_io, retired, primary_error;
  uintptr_t attach;
  uint32_t cleanup_count, cleanup_codes[8];
  uint32_t primary_error_domain;
  uint64_t omitted_cleanup;
  uint32_t cleanup_state, cleanup_error_domains;
  int64_t cleanup_start, cleanup_grace, cleanup_total, counter_frequency;
};
struct SsCaptureAttachProgressV2 {
  uint32_t abi_version, byte_size;
  uintptr_t attach;
  SsCaptureCleanupSourceV1 cleanup;
  SsCaptureIssueSourceV2 issue;
};
// Stage: 1 open pipe, 2 initialize globals, 3 issue registration,
// 4 validate response/events, 5 install handlers, 6 publish native fact.
// Every token is usable only while its exact source callback is active on the
// original worker thread; nonce identifies this exact one-shot stage request.
struct SsCaptureAttachStepV2 {
  uint32_t abi_version, byte_size;
  uintptr_t attach;
  uint32_t stage, reserved;
  uint64_t nonce;
};
static_assert(sizeof(uintptr_t) == 8);
static_assert(sizeof(SsCaptureCleanupBudgetV1) == 48);
static_assert(sizeof(SsCaptureCleanupSourceV1) == 16);
static_assert(sizeof(SsCaptureIssueSourceV2) == 16);
static_assert(sizeof(SsCaptureAttachInputV2) == 776);
static_assert(offsetof(SsCaptureAttachInputV2, cleanup) == 736);
static_assert(offsetof(SsCaptureAttachInputV2, issue) == 752);
static_assert(offsetof(SsCaptureAttachInputV2, borrowed_completion_event) == 768);
static_assert(sizeof(SsCaptureAttachResultV2) == 120);
static_assert(offsetof(SsCaptureAttachResultV2, attach) == 24);
static_assert(offsetof(SsCaptureAttachResultV2, omitted_cleanup) == 72);
static_assert(sizeof(SsCaptureAttachProgressV2) == 48);
static_assert(sizeof(SsCaptureAttachStepV2) == 32);
extern "C" __declspec(dllexport) void ss_capture_attach_v2(
    const SsCaptureAttachInputV2*, SsCaptureAttachResultV2*);
extern "C" __declspec(dllexport) void ss_capture_attach_poll_v2(
    const SsCaptureAttachProgressV2*, SsCaptureAttachResultV2*);
extern "C" __declspec(dllexport) void ss_capture_attach_step_v2(
    const SsCaptureAttachStepV2*, SsCaptureAttachResultV2*);
extern "C" __declspec(dllexport) void ss_capture_attach_retire_v2(
    const SsCaptureAttachProgressV2*, SsCaptureAttachResultV2*);

#endif
