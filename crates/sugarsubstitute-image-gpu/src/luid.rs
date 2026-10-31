// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Identify one physical adapter across graphics APIs by its operating-system LUID.

/// Locally unique adapter identity reported by both Vulkan and DXGI for the current boot.
///
/// Vulkan reports `VkPhysicalDeviceIDProperties::deviceLUID` as the in-memory bytes of the
/// Windows `LUID` structure: a little-endian low `u32` followed by a little-endian high `i32`.
/// The value is process-local evidence; it is never persisted or used across restarts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AdapterLuid {
    low: u32,
    high: i32,
}

impl AdapterLuid {
    /// Bind the two halves of a DXGI `LUID`.
    pub const fn from_parts(low: u32, high: i32) -> Self {
        Self { low, high }
    }
    /// Decode Vulkan's eight-byte `deviceLUID` representation.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self {
            low: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            high: i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        }
    }
    /// Low half of the DXGI `LUID`.
    pub const fn low(self) -> u32 {
        self.low
    }
    /// High half of the DXGI `LUID`.
    pub const fn high(self) -> i32 {
        self.high
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_luid_bytes_decode_to_the_dxgi_structure_halves() {
        let luid = AdapterLuid::from_bytes([0x78, 0x56, 0x34, 0x12, 0xfe, 0xff, 0xff, 0xff]);
        assert_eq!(luid, AdapterLuid::from_parts(0x1234_5678, -2));
        assert_eq!((luid.low(), luid.high()), (0x1234_5678, -2));
        assert_ne!(luid, AdapterLuid::from_parts(0x1234_5678, 0));
    }
}
