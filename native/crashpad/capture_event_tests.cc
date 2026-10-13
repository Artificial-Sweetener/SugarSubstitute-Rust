// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_event.h"
#include <cstdio>
#include <cstring>

namespace {
struct Boundary {
  NTSTATUS basic_status = 0, type_status = 0;
  ACCESS_MASK access = SYNCHRONIZE | EVENT_MODIFY_STATE;
  bool short_basic = false, oversized = false, short_type = false;
  bool outside = false, unaligned = false, large_capacity = false, wrong_type = false;
  int calls = 0;
  int counters = 0;
  int64_t now = 100, basic_returns_at = 0;
} boundary;
NTSTATUS NTAPI Query(HANDLE, OBJECT_INFORMATION_CLASS kind, PVOID output, ULONG size, PULONG length) {
  ++boundary.calls;
  if (kind == ObjectBasicInformation) {
    auto* basic = static_cast<PUBLIC_OBJECT_BASIC_INFORMATION*>(output);
    basic->GrantedAccess = boundary.access;
    *length = boundary.short_basic ? 0 : sizeof(*basic);
    if (boundary.basic_returns_at) boundary.now = boundary.basic_returns_at;
    return boundary.basic_status;
  }
  auto* type = static_cast<PUBLIC_OBJECT_TYPE_INFORMATION*>(output);
  type->TypeName.Buffer = reinterpret_cast<wchar_t*>(static_cast<unsigned char*>(output) + sizeof(*type));
  const wchar_t* text = boundary.wrong_type ? L"Token" : L"Event";
  std::memcpy(type->TypeName.Buffer, text, 5 * sizeof(wchar_t));
  type->TypeName.Length = 10;
  type->TypeName.MaximumLength = boundary.large_capacity ? 1000 : 10;
  *length = boundary.oversized ? size + 1 : boundary.short_type ? 1 : sizeof(*type) + 10;
  if (boundary.outside) type->TypeName.Buffer = reinterpret_cast<wchar_t*>(1);
  if (boundary.unaligned) type->TypeName.Buffer = reinterpret_cast<wchar_t*>(reinterpret_cast<uintptr_t>(type->TypeName.Buffer) + 1);
  return boundary.type_status;
}
const sugarsubstitute::CaptureEventApi api = {&Query};
bool Inspect(sugarsubstitute::OwnedConnection& owner) {
  return sugarsubstitute::InspectCaptureEvent(owner, reinterpret_cast<HANDLE>(10), nullptr, &api);
}
bool ExactTypeAndAccess() {
  boundary = {}; sugarsubstitute::OwnedConnection owner;
  return Inspect(owner) && !owner.error && boundary.calls == 2;
}
bool MissingAccessPreventsTypeQuery() {
  boundary = {}; boundary.access = SYNCHRONIZE; sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == ERROR_ACCESS_DENIED && boundary.calls == 1;
}
bool ShortBasic() {
  boundary = {}; boundary.short_basic = true; sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == ERROR_INVALID_DATA && boundary.calls == 1;
}
bool FailedBasicStatusPreserved() {
  boundary = {}; boundary.basic_status = static_cast<NTSTATUS>(0xc0000008u);
  sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == 0xc0000008u &&
      owner.error_domain == sugarsubstitute::NativeCauseDomain::Ntstatus;
}
bool FailedTypeStatusPreserved() {
  boundary = {}; boundary.type_status = static_cast<NTSTATUS>(0xc0000022u);
  sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == 0xc0000022u &&
      owner.error_domain == sugarsubstitute::NativeCauseDomain::Ntstatus;
}
bool Shape(bool Boundary::*field) {
  boundary = {}; boundary.*field = true; sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == ERROR_INVALID_DATA;
}
bool Oversized() { return Shape(&Boundary::oversized); }
bool Truncated() { return Shape(&Boundary::short_type); }
bool OutsidePointer() { return Shape(&Boundary::outside); }
bool UnalignedPointer() { return Shape(&Boundary::unaligned); }
bool CapacityBeyondReply() { return Shape(&Boundary::large_capacity); }
bool TokenCannotBecomeEvent() {
  boundary = {}; boundary.wrong_type = true; sugarsubstitute::OwnedConnection owner;
  return !Inspect(owner) && owner.error == ERROR_ACCESS_DENIED;
}
BOOL WINAPI Counter(LARGE_INTEGER* output) {
  ++boundary.counters; output->QuadPart = boundary.now; return TRUE;
}
const sugarsubstitute::ConnectionApi clock_api = {&Counter, nullptr, nullptr, nullptr,
    nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr};
bool BasicLateReturnVetoesTypeQuery() {
  boundary = {}; boundary.basic_returns_at = 15100;
  sugarsubstitute::OwnedConnection owner; owner.boundary = &clock_api;
  owner.error = 22; owner.error_domain = sugarsubstitute::NativeCauseDomain::Errno;
  sugarsubstitute::CleanupLimit limit; limit.start_counter = limit.previous_counter = 100;
  limit.total_counter = 15100;
  return !sugarsubstitute::InspectCaptureEvent(owner, reinterpret_cast<HANDLE>(10), &limit, &api) &&
      boundary.calls == 1 && boundary.counters == 2 && limit.stopped &&
      owner.error == 22 && owner.error_domain == sugarsubstitute::NativeCauseDomain::Errno &&
      owner.cleanup_count == 1 && owner.cleanup_codes[0] == ERROR_TIMEOUT;
}
bool PositiveBasicReturnRegressionVetoesTypeQuery() {
  boundary = {}; boundary.basic_returns_at = 99;
  sugarsubstitute::OwnedConnection owner; owner.boundary = &clock_api;
  sugarsubstitute::CleanupLimit limit; limit.start_counter = limit.previous_counter = 100;
  limit.total_counter = 15100;
  return !sugarsubstitute::InspectCaptureEvent(owner, reinterpret_cast<HANDLE>(10), &limit, &api) &&
      boundary.calls == 1 && boundary.counters == 2 && limit.stopped &&
      owner.cleanup_codes[0] == ERROR_INVALID_DATA && limit.previous_counter == 100;
}
bool OriginalStartFloorVetoesEvenBasicQuery() {
  boundary = {}; sugarsubstitute::OwnedConnection owner; owner.boundary = &clock_api;
  sugarsubstitute::CleanupLimit limit; limit.start_counter = limit.previous_counter = 101;
  limit.total_counter = 15100;
  return !sugarsubstitute::InspectCaptureEvent(owner, reinterpret_cast<HANDLE>(10), &limit, &api) &&
      !boundary.calls && boundary.counters == 1 && limit.stopped &&
      owner.cleanup_codes[0] == ERROR_INVALID_DATA;
}
}  // namespace
int main() {
  const struct {const char* name; bool(*test)();} cases[] = {
    {"exact-type-and-required-rights", ExactTypeAndAccess},
    {"missing-rights-before-type-query", MissingAccessPreventsTypeQuery},
    {"short-basic", ShortBasic}, {"basic-raw-native-status", FailedBasicStatusPreserved},
    {"type-raw-native-status", FailedTypeStatusPreserved}, {"oversized", Oversized},
    {"truncated", Truncated}, {"outside-pointer", OutsidePointer},
    {"unaligned-pointer", UnalignedPointer}, {"capacity-outside-reply", CapacityBeyondReply},
    {"token-is-not-event", TokenCannotBecomeEvent},
    {"basic-return-at-cutoff-no-type-query", BasicLateReturnVetoesTypeQuery},
    {"basic-positive-counter-regression-no-type-query", PositiveBasicReturnRegressionVetoesTypeQuery},
    {"counter-before-original-start-no-basic-query", OriginalStartFloorVetoesEvenBasicQuery},
  };
  for (const auto& item : cases) {
    if (!item.test()) { std::fprintf(stderr, "FAIL %s\n", item.name); return 1; }
  }
  std::puts("14 CPU-only actual Event inspection cases PASS; no native object queries");
  return 0;
}
