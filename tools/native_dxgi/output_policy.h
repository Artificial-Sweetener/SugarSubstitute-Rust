// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#pragma once
#include <cstdint>

namespace ss::dxgi {
// Original DuplicateOutput owns any OS BGRA conversion. These two enrolled output
// modes do not grant HDR-gamut fidelity: real acquired texture must still be BGRA8,
// and Root must qualify actual known-color pixels against the independent GDI path.
constexpr bool accepted_color(uint32_t color_space, uint32_t bits, uint32_t flags) noexcept {
    if ((flags & ~uint32_t{15}) != 0)
        return false;
    if (color_space == 0 && bits == 8)
        return (flags & 6) == 0;
    if (color_space == 12 && bits == 10)
        return flags == 2 || flags == 3;
    return false;
}
} // namespace ss::dxgi
