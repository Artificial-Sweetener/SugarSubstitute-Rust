// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include "observer.h"
#include <stdexcept>
namespace ss::dxgi::tests {
inline void check(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
inline ss_dxgi_request request() { return {1, sizeof(ss_dxgi_request), -8, -4, 2, 2, 5, 5}; }
} // namespace ss::dxgi::tests
