// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "sugarsubstitute/capture_event.h"
#include <cstring>

namespace sugarsubstitute {
namespace {
const CaptureEventApi system_api = {&NtQueryObject};
}
bool InspectCaptureEvent(OwnedConnection& owner, HANDLE event,
                         CleanupLimit* limit, const CaptureEventApi* boundary) {
  const auto& api = boundary ? *boundary : system_api;
  PUBLIC_OBJECT_BASIC_INFORMATION basic = {};
  ULONG length = 0;
  if (!owner.CleanupAllowed(limit)) return false;
  NTSTATUS status = api.query(event, ObjectBasicInformation, &basic, sizeof(basic), &length);
  if (status < 0) return owner.FailStatus(status);
  if (length != sizeof(basic)) return owner.Fail(ERROR_INVALID_DATA);
  constexpr ACCESS_MASK required = SYNCHRONIZE | EVENT_MODIFY_STATE;
  if ((basic.GrantedAccess & required) != required) return owner.Fail(ERROR_ACCESS_DENIED);
  alignas(void*) unsigned char storage[1024] = {};
  if (!owner.CleanupAllowed(limit)) return false;
  status = api.query(event, ObjectTypeInformation, storage, sizeof(storage), &length);
  if (status < 0) return owner.FailStatus(status);
  if (length < sizeof(PUBLIC_OBJECT_TYPE_INFORMATION) || length > sizeof(storage))
    return owner.Fail(ERROR_INVALID_DATA);
  const auto* type = reinterpret_cast<const PUBLIC_OBJECT_TYPE_INFORMATION*>(storage);
  constexpr wchar_t expected[] = L"Event";
  constexpr size_t name_bytes = sizeof(expected) - sizeof(wchar_t);
  const auto start = reinterpret_cast<uintptr_t>(storage);
  const auto pointer = reinterpret_cast<uintptr_t>(type->TypeName.Buffer);
  if (type->TypeName.Length != name_bytes || type->TypeName.MaximumLength < name_bytes ||
      type->TypeName.MaximumLength > length || pointer % alignof(wchar_t) ||
      pointer < start + sizeof(*type) || pointer > start + length - name_bytes ||
      type->TypeName.MaximumLength > start + length - pointer)
    return owner.Fail(ERROR_INVALID_DATA);
  if (std::memcmp(type->TypeName.Buffer, expected, name_bytes) != 0)
    return owner.Fail(ERROR_ACCESS_DENIED);
  return true;
}
}  // namespace sugarsubstitute
