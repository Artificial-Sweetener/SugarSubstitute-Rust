// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include <Windows.h>
namespace ss::dxgi {
// A failed BOOL must stay failed even when its API leaves LastError unset.
inline HRESULT win32_failure(DWORD error) noexcept {
    return error ? HRESULT_FROM_WIN32(error) : E_FAIL;
}
} // namespace ss::dxgi
