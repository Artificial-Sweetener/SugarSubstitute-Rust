// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_OWNED_START_H_
#define SUGARSUBSTITUTE_OWNED_START_H_

#include <windows.h>
#include <stdint.h>
#include <string>
#include "sugarsubstitute/owned_connection.h"

namespace sugarsubstitute {

// One process-lifetime registration owner. Pending OVERLAPPED storage is never
// destroyed on cancellation; subsequent finite retirement calls observe it.
struct OwnedStart {
  HANDLE process = nullptr;
  HANDLE thread = nullptr;
  HANDLE initial_pipe = nullptr;
  HANDLE bootstrap_process = nullptr;
  OwnedConnection connection;
  DWORD creator_pid = 0;
  DWORD handler_pid = 0;
  uint64_t creator_birth = 0;
  uint64_t handler_birth = 0;
  bool prepared = false;
  bool completed = false;
};

// Acquire and authenticate the connected inherited-instance capability before
// child creation. Failure retains every acquired handle for checked retirement.
bool PrepareInheritedRegistration(const std::wstring& name, OwnedStart* owner);
// Confirm the already-connected endpoint with the actual retained live child.
// This never discovers another instance by name or renews the original deadline.
bool ConfirmHandler(OwnedStart* owner);
// Returns false while actual cancellation completion/checked closure is pending.
bool RetireRegistration(OwnedStart* owner);

}  // namespace sugarsubstitute
#endif
