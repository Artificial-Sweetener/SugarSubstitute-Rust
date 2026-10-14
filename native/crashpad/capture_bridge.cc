// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_bridge.h"
#include "sugarsubstitute/owned_start.h"
#include "sugarsubstitute/capture_source.h"
#include "sugarsubstitute/capture_attach_boundary.h"
#include "sugarsubstitute/capture_event.h"
#include "client/crash_report_database.h"
#include "client/crashpad_client.h"
#include "client/crashpad_info.h"
#include "client/settings.h"
#include "client/simple_string_dictionary.h"

#include <algorithm>
#include <atomic>
#include <map>
#include <string>

namespace {
// Crashpad installs callbacks referencing these exact addresses. Intentional
// process-lifetime storage cannot be freed by logical capture-owner shutdown.
crashpad::CrashpadClient client;
crashpad::SimpleStringDictionary annotations;
sugarsubstitute::OwnedStart retained;
sugarsubstitute::CaptureSource source;
std::atomic<bool> attempted{false};
sugarsubstitute::OwnedAttach captured;

void HexBytes(const uint8_t* bytes, size_t count, char* output) {
  constexpr char hex[] = "0123456789abcdef";
  for (size_t i = 0; i < count; ++i) { output[2 * i] = hex[bytes[i] >> 4]; output[2 * i + 1] = hex[bytes[i] & 15]; }
}
void HexNumber(uint64_t value, char* output) {
  constexpr char hex[] = "0123456789abcdef";
  for (size_t i = 0; i < 16; ++i) { output[15 - i] = hex[value & 15]; value >>= 4; }
}
bool InitializeCaptured(sugarsubstitute::OwnedAttach* owner) {
  // These are the exact accepted native attach owner fields, not inferred PID
  // or endpoint spelling. The retained dictionary copies fixed string views.
  char coordinator[97] = {}, subject[134] = {};
  HexBytes(owner->coordinator_namespace, 32, coordinator);
  HexBytes(owner->coordinator_epoch, 16, coordinator + 64);
  memcpy(subject, owner->subject == 1 ? "d1:" : "s1:", 3);
  memcpy(subject + 3, coordinator, 96);
  if (owner->subject == 1) {
    subject[99] = ':'; HexNumber(owner->instance, subject + 100);
    subject[116] = ':'; HexNumber(owner->generation, subject + 117);
  }
  annotations.SetKeyValue("coordinator", coordinator);
  annotations.SetKeyValue("subject", subject);
  annotations.SetKeyValue("product", "SugarSubstitute");
  annotations.SetKeyValue("build", owner->build);
  annotations.SetKeyValue("role", owner->subject == 1 ? "desktop" : "supervisor");
  crashpad::CrashpadInfo::GetCrashpadInfo()->set_simple_annotations(&annotations);
  return client.InitializeCapturedAttach(owner);
}
bool InspectCapturedEvent(sugarsubstitute::OwnedAttach* owner, size_t index) {
  return sugarsubstitute::InspectCaptureEvent(owner->connection, owner->response_events[index],
      owner->stopped ? &owner->cleanup_limit : nullptr);
}
bool CommitCaptured(sugarsubstitute::OwnedAttach* owner) {
  return client.CommitCapturedEvents(owner);
}
bool InstallCaptured(sugarsubstitute::OwnedAttach* owner) {
  return client.InstallCapturedHandlers(owner);
}
const sugarsubstitute::AttachRuntime captured_runtime = {
  &InitializeCaptured, &InspectCapturedEvent, &CommitCaptured, &InstallCaptured
};
sugarsubstitute::CaptureAttachBoundary captured_boundary(captured, attempted, captured_runtime);
static_assert(sizeof(client) + sizeof(annotations) + sizeof(retained) + sizeof(source) + sizeof(captured) + sizeof(captured_boundary) + sizeof(captured_runtime) + sizeof(attempted) <= 48 * 1024,
              "Pinned native callback objects exceed the reviewed storage bound");

bool ValidPath(const uint16_t* value, size_t length) {
  if (!value || !length || length >= 32767) return false;
  for (size_t i = 0; i < length; ++i) if (!value[i]) return false;
  return true;
}
bool ValidIdentity(const char* value, size_t length, size_t maximum) {
  if (!value || !length || length > maximum) return false;
  for (size_t i = 0; i < length; ++i)
    if (!(value[i] >= '0' && value[i] <= '9') &&
        !(value[i] >= 'a' && value[i] <= 'z') &&
        !(value[i] >= 'A' && value[i] <= 'Z') &&
        value[i] != '-' && value[i] != '_') return false;
  return true;
}
base::FilePath Path(const uint16_t* value, size_t length) {
  static_assert(sizeof(wchar_t) == sizeof(uint16_t));
  return base::FilePath(std::wstring(reinterpret_cast<const wchar_t*>(value), length));
}
void Facts(SsCaptureResult* result, bool registered) {
  *result = {1, registered ? 1u : 0u, retained.connection.error,
             retained.connection.cleanup_error, retained.connection.pending ? 1u : 0u, 0,
             reinterpret_cast<uintptr_t>(retained.process),
             reinterpret_cast<uintptr_t>(retained.thread)};
  // Partial CreateProcess ownership crosses the ABI before status is interpreted.
  retained.process = nullptr;
  retained.thread = nullptr;
  result->cleanup_count = static_cast<uint32_t>(retained.connection.cleanup_count);
  for (size_t i = 0; i < retained.connection.cleanup_count; ++i)
    result->cleanup_codes[i] = retained.connection.cleanup_codes[i];
  result->omitted_cleanup = retained.connection.omitted_cleanup;
  retained.connection.cleanup_count = 0;
  retained.connection.omitted_cleanup = 0;
}
}  // namespace

void ss_capture_start_v1(const SsCaptureInput* input, SsCaptureResult* result) {
  if (!result) return;
  *result = {1, 0, ERROR_INVALID_PARAMETER, 0, 0, 0, 0, 0};
  if (!input || !ValidPath(input->handler, input->handler_length) ||
      !ValidPath(input->database, input->database_length) ||
      !ValidPath(input->metrics, input->metrics_length) ||
      !ValidIdentity(input->build, input->build_length, 64) ||
      !ValidIdentity(input->coordinator, input->coordinator_length, 96)) return;
  LARGE_INTEGER frequency = {};
  if (!QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0 ||
      input->counter_frequency != frequency.QuadPart) return;
  retained.connection.deadline_counter = input->deadline_counter;
  retained.connection.frequency = input->counter_frequency;
  if (!retained.connection.CheckDeadline()) { Facts(result, false); return; }
  bool expected = false;
  if (!attempted.compare_exchange_strong(expected, true)) {
    result->error = ERROR_ALREADY_INITIALIZED;
    return;
  }
  const auto database_path = Path(input->database, input->database_length);
  auto database = crashpad::CrashReportDatabase::Initialize(database_path);
  if (!database) {
    retained.connection.Fail(GetLastError());
    Facts(result, false);
    return;
  }
  if (!database->GetSettings()->SetUploadsEnabled(false)) {
    retained.connection.Fail(GetLastError());
    Facts(result, false);
    return;
  }
  const std::string build(input->build, input->build_length);
  const std::string coordinator(input->coordinator, input->coordinator_length);
  annotations.SetKeyValue("product", "SugarSubstitute");
  annotations.SetKeyValue("build", build);
  annotations.SetKeyValue("role", "supervisor");
  annotations.SetKeyValue("coordinator", coordinator);
  if (coordinator.size() == 96 && std::all_of(coordinator.begin(), coordinator.end(), [](char c) {
        return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f');
      })) {
    char subject[100] = {};
    memcpy(subject, "s1:", 3); memcpy(subject + 3, coordinator.data(), 96);
    annotations.SetKeyValue("subject", subject);
  }
  crashpad::CrashpadInfo::GetCrashpadInfo()->set_simple_annotations(&annotations);
  const bool registered = client.StartHandlerOwned(
      Path(input->handler, input->handler_length), database_path,
      Path(input->metrics, input->metrics_length),
      {{"product", "SugarSubstitute"}, {"build", build}}, &retained);
  if (!registered && !retained.connection.error) {
    retained.connection.Fail(GetLastError());
  }
  if (registered && !source.Publish(retained, client.GetHandlerIPCPipe()))
    retained.connection.Fail(ERROR_INVALID_DATA);
  Facts(result, registered);
}

void ss_capture_retire_io_v1(SsCaptureResult* result) {
  if (!result) return;
  if (captured.started) {
    *result = {1, 0, ERROR_INVALID_STATE, 0, 0, 0, 0, 0};
    return;
  }
  source.Revoke();
  const bool closed = sugarsubstitute::RetireRegistration(&retained);
  Facts(result, false);
  result->retired = closed ? 1u : 0u;
  if (!closed && !result->cleanup_error) result->cleanup_error = ERROR_IO_INCOMPLETE;
}

bool ss_capture_endpoint_v1(const SsCaptureEndpointExpectationV1* expected,
                            SsCaptureEndpointV1* output) {
  return source.Endpoint(expected, output);
}
bool ss_capture_source_identity_v1(SsCaptureSourceIdentityV1* output) {
  return source.Identity(output);
}

void ss_capture_attach_v2(const SsCaptureAttachInputV2* input,
                           SsCaptureAttachResultV2* output) {
  captured_boundary.Start(input, output);
}
void ss_capture_attach_poll_v2(const SsCaptureAttachProgressV2* progress,
                                SsCaptureAttachResultV2* output) {
  captured_boundary.Poll(progress, output);
}
void ss_capture_attach_step_v2(const SsCaptureAttachStepV2* step,
                                SsCaptureAttachResultV2* output) {
  captured_boundary.Step(step, output);
}
void ss_capture_attach_retire_v2(const SsCaptureAttachProgressV2* progress,
                                  SsCaptureAttachResultV2* output) {
  captured_boundary.Retire(progress, output);
}
