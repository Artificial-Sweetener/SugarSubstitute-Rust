// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_bridge.h"
#include "sugarsubstitute/owned_start.h"
#include "client/crash_report_database.h"
#include "client/crashpad_client.h"
#include "client/crashpad_info.h"
#include "client/settings.h"
#include "client/simple_string_dictionary.h"

#include <atomic>
#include <map>
#include <string>

namespace {
// Crashpad installs callbacks referencing these exact addresses. Intentional
// process-lifetime storage cannot be freed by logical capture-owner shutdown.
crashpad::CrashpadClient client;
crashpad::SimpleStringDictionary annotations;
sugarsubstitute::OwnedStart retained;
std::atomic<bool> attempted{false};
static_assert(sizeof(client) + sizeof(annotations) + sizeof(retained) <= 48 * 1024,
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
  *result = {1, registered ? 1u : 0u, retained.error,
             retained.cleanup_error, retained.pending ? 1u : 0u, 0,
             reinterpret_cast<uintptr_t>(retained.process),
             reinterpret_cast<uintptr_t>(retained.thread)};
  // Partial CreateProcess ownership crosses the ABI before status is interpreted.
  retained.process = nullptr;
  retained.thread = nullptr;
  result->cleanup_count = static_cast<uint32_t>(retained.cleanup_count);
  for (size_t i = 0; i < retained.cleanup_count; ++i)
    result->cleanup_codes[i] = retained.cleanup_codes[i];
  result->omitted_cleanup = retained.omitted_cleanup;
  retained.cleanup_count = 0;
  retained.omitted_cleanup = 0;
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
  retained.deadline_counter = input->deadline_counter;
  retained.frequency = input->counter_frequency;
  if (!sugarsubstitute::CheckDeadline(&retained)) { Facts(result, false); return; }
  bool expected = false;
  if (!attempted.compare_exchange_strong(expected, true)) {
    result->error = ERROR_ALREADY_INITIALIZED;
    return;
  }
  const auto database_path = Path(input->database, input->database_length);
  auto database = crashpad::CrashReportDatabase::Initialize(database_path);
  if (!database) {
    retained.error = GetLastError();
    if (!retained.error) retained.error = ERROR_GEN_FAILURE;
    Facts(result, false);
    return;
  }
  if (!database->GetSettings()->SetUploadsEnabled(false)) {
    retained.error = GetLastError();
    if (!retained.error) retained.error = ERROR_GEN_FAILURE;
    Facts(result, false);
    return;
  }
  const std::string build(input->build, input->build_length);
  const std::string coordinator(input->coordinator, input->coordinator_length);
  annotations.SetKeyValue("product", "SugarSubstitute");
  annotations.SetKeyValue("build", build);
  annotations.SetKeyValue("role", "supervisor");
  annotations.SetKeyValue("coordinator", coordinator);
  crashpad::CrashpadInfo::GetCrashpadInfo()->set_simple_annotations(&annotations);
  const bool registered = client.StartHandlerOwned(
      Path(input->handler, input->handler_length), database_path,
      Path(input->metrics, input->metrics_length),
      {{"product", "SugarSubstitute"}, {"build", build}}, &retained);
  if (!registered && !retained.error) {
    retained.error = GetLastError();
    if (!retained.error) retained.error = ERROR_GEN_FAILURE;
  }
  Facts(result, registered);
}

void ss_capture_retire_io_v1(SsCaptureResult* result) {
  if (!result) return;
  const bool closed = sugarsubstitute::RetireRegistration(&retained);
  Facts(result, false);
  result->retired = closed ? 1u : 0u;
  if (!closed && !result->cleanup_error) result->cleanup_error = ERROR_IO_INCOMPLETE;
}
