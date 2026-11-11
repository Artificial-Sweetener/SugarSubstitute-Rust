// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// CPU-only tests of the actual cache; constructed matching facts confer no authority.
#include "sugarsubstitute/capture_source.h"
#include <cstdio>

namespace {
sugarsubstitute::OwnedStart Registered() {
  sugarsubstitute::OwnedStart owner;
  owner.creator_pid = 61; owner.handler_pid = 71;
  owner.creator_birth = 101; owner.handler_birth = 202;
  owner.completed = true;
  return owner;
}
SsCaptureEndpointExpectationV1 Expectation() { return {1, 61, 101, 71, 0, 202}; }
bool Check(bool condition, const char* name) {
  if (!condition) std::fprintf(stderr, "FAIL %s\n", name);
  return condition;
}
bool MissingIsUnavailable() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); owner.completed = false;
  SsCaptureSourceIdentityV1 identity = {99, 99, 99, 99, 99, 99, {99, 99}};
  return Check(!cache.Publish(owner, L"\\\\.\\pipe\\actual") &&
      !cache.Identity(&identity) && identity.abi_version == 1 && !identity.status &&
      !identity.creator_pid && !identity.creator_birth && !identity.reserved[0],
      "absence-initializes-all-output-and-never-becomes-registration");
}
bool PendingCannotPublish() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); owner.connection.pending = true;
  return Check(!cache.Publish(owner, L"\\\\.\\pipe\\actual"),
      "pending-io-is-not-checked-source-retirement");
}
bool TemporaryCannotPublish() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); owner.bootstrap_process = reinterpret_cast<HANDLE>(1);
  return Check(!cache.Publish(owner, L"\\\\.\\pipe\\actual"),
      "live-bootstrap-slot-prevents-source-publication");
}
bool ExactFactsAndTail() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); auto expected = Expectation();
  SsCaptureEndpointV1 endpoint = {};
  SsCaptureSourceIdentityV1 identity = {};
  if (!cache.Publish(owner, L"\\\\.\\pipe\\actual")) return false;
  bool zero_tail = true;
  if (!cache.Endpoint(&expected, &endpoint)) return false;
  for (size_t i = endpoint.length; i < 256; ++i) zero_tail &= !endpoint.endpoint[i];
  return Check(cache.Identity(&identity) && identity.creator_birth == 101 &&
      identity.handler_birth == 202 && identity.creator_pid == 61 &&
      identity.handler_pid == 71 && endpoint.provenance == 1 && endpoint.status == 1 &&
      endpoint.length == 15 && !endpoint.error && zero_tail,
      "cache-only-getters-return-exact-original-source-and-zero-tail");
}
bool ForeignBirthRejected() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); auto expected = Expectation(); expected.handler_birth = 203;
  SsCaptureEndpointV1 endpoint = {};
  if (!cache.Publish(owner, L"\\\\.\\pipe\\actual")) return false;
  return Check(!cache.Endpoint(&expected, &endpoint) &&
      endpoint.error == ERROR_ACCESS_DENIED && !endpoint.status && !endpoint.length &&
      !endpoint.creator_pid && !endpoint.endpoint[0],
      "same-pid-foreign-birth-cannot-read-source-match");
}
bool ReservedExpectationRejected() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); auto expected = Expectation(); expected.reserved = 1;
  SsCaptureEndpointV1 endpoint = {};
  if (!cache.Publish(owner, L"\\\\.\\pipe\\actual")) return false;
  return Check(!cache.Endpoint(&expected, &endpoint) &&
      endpoint.error == ERROR_INVALID_PARAMETER && !endpoint.status,
      "nonzero-expectation-reserved-is-not-compatible");
}
bool MalformedEndpointRejected() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered();
  return Check(!cache.Publish(owner, L"\\\\foreign\\pipe\\actual") &&
      !cache.Publish(owner, std::wstring(256, L'x')) &&
      !cache.Publish(owner, std::wstring(L"\\\\.\\pipe\\a\0b", 12)),
      "foreign-oversize-and-embedded-nul-endpoints-reject-before-publication");
}
bool RevocationIsPermanent() {
  sugarsubstitute::CaptureSource cache;
  auto owner = Registered(); auto expected = Expectation();
  SsCaptureEndpointV1 endpoint = {}; SsCaptureSourceIdentityV1 identity = {};
  if (!cache.Publish(owner, L"\\\\.\\pipe\\actual")) return false;
  cache.Revoke();
  return Check(!cache.Identity(&identity) && !identity.status &&
      !cache.Endpoint(&expected, &endpoint) && endpoint.error == ERROR_INVALID_STATE &&
      !cache.Publish(owner, L"\\\\.\\pipe\\replacement"),
      "revoked-source-cannot-republish-or-substitute-original-facts");
}
}  // namespace
int main() {
  const bool passed = MissingIsUnavailable() && PendingCannotPublish() &&
      TemporaryCannotPublish() && ExactFactsAndTail() && ForeignBirthRejected() &&
      ReservedExpectationRejected() && MalformedEndpointRejected() && RevocationIsPermanent();
  if (passed) std::puts("8 CPU-only actual source-cache cases PASS; no native calls");
  return passed ? 0 : 1;
}
