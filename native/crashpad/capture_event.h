// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_EVENT_H_
#define SUGARSUBSTITUTE_CAPTURE_EVENT_H_
#include "sugarsubstitute/owned_connection.h"
#include <winternl.h>

namespace sugarsubstitute {
// Fake only the external object query; the same checked owner validates its reply.
struct CaptureEventApi { decltype(&NtQueryObject) query; };
// Finite object inspection runs on the original worker outside all owner locks.
// Success establishes type/access only; caller still owns adoption and closure.
bool InspectCaptureEvent(OwnedConnection& owner, HANDLE event,
                         CleanupLimit* limit, const CaptureEventApi* boundary = nullptr);
}  // namespace sugarsubstitute
#endif
