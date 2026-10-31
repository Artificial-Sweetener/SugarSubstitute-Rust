// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Place the composition device on the adapter that can share textures with the canvas GPU.
//!
//! Hybrid systems commonly report an integrated adapter as the default D3D11 device while the
//! canvas renders on a discrete Vulkan device. Placement therefore follows the selected Vulkan
//! device's LUID, never the default adapter, and falls back to CPU frames only for a named reason.
use std::fmt;
use sugarsubstitute_image_gpu::{AdapterDescriptor, AdapterLuid};

/// Why the embedded canvas presents CPU-prepared frames instead of shared GPU textures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedTextureFallback {
    /// The canvas session selected no GPU renderer.
    NoSelectedGpu,
    /// The selected Vulkan driver reports no valid adapter LUID.
    MissingLuid,
    /// The selected Vulkan driver lacks `VK_KHR_external_memory_win32` texture import.
    MissingExternalMemory,
    /// No hardware D3D11 adapter has the selected Vulkan device's LUID.
    NoMatchingAdapter,
}

impl SharedTextureFallback {
    /// Stable structured-log value for this fallback reason.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoSelectedGpu => "no_selected_gpu",
            Self::MissingLuid => "missing_vulkan_luid",
            Self::MissingExternalMemory => "missing_vulkan_external_memory",
            Self::NoMatchingAdapter => "no_matching_d3d11_adapter",
        }
    }
}

impl fmt::Display for SharedTextureFallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Adapter placement for one composition graphics device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionAdapter {
    /// Create the device on this adapter; shared GPU textures are available.
    Shared(AdapterLuid),
    /// Create the device on the default hardware adapter and present CPU frames.
    Default(SharedTextureFallback),
}

/// Choose the composition adapter for the selected canvas GPU.
///
/// `hardware` lists the LUIDs of every enumerated hardware D3D11 adapter, in any order; the
/// default adapter's position is irrelevant.
pub fn composition_adapter(
    selected: Option<&AdapterDescriptor>,
    hardware: &[AdapterLuid],
) -> CompositionAdapter {
    let Some(selected) = selected else {
        return CompositionAdapter::Default(SharedTextureFallback::NoSelectedGpu);
    };
    let Some(luid) = selected.luid else {
        return CompositionAdapter::Default(SharedTextureFallback::MissingLuid);
    };
    if !selected.shared_textures {
        return CompositionAdapter::Default(SharedTextureFallback::MissingExternalMemory);
    }
    if !hardware.contains(&luid) {
        return CompositionAdapter::Default(SharedTextureFallback::NoMatchingAdapter);
    }
    CompositionAdapter::Shared(luid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_image_gpu::{AdapterClass, AdapterId, Backend};

    fn descriptor(luid: Option<AdapterLuid>, shared_textures: bool) -> AdapterDescriptor {
        AdapterDescriptor {
            id: AdapterId::fixture(0),
            name: "Discrete GPU".into(),
            vendor: 0x10de,
            device: 0x2b85,
            backend: Backend::Vulkan,
            class: AdapterClass::Discrete,
            compatible: true,
            luid,
            shared_textures,
        }
    }

    const INTEGRATED: AdapterLuid = AdapterLuid::from_parts(0x1000, 0);
    const DISCRETE: AdapterLuid = AdapterLuid::from_parts(0x2000, 0);

    #[test]
    fn hybrid_systems_place_composition_on_the_selected_vulkan_adapter_not_the_default() {
        let selected = descriptor(Some(DISCRETE), true);
        for hardware in [[INTEGRATED, DISCRETE], [DISCRETE, INTEGRATED]] {
            assert_eq!(
                composition_adapter(Some(&selected), &hardware),
                CompositionAdapter::Shared(DISCRETE)
            );
        }
    }

    #[test]
    fn each_unavailable_interop_requirement_reports_its_own_fallback() {
        let fallback = |selected: Option<&AdapterDescriptor>, hardware: &[AdapterLuid]| {
            match composition_adapter(selected, hardware) {
                CompositionAdapter::Default(reason) => Some(reason),
                CompositionAdapter::Shared(_) => None,
            }
        };
        assert_eq!(fallback(None, &[DISCRETE]), Some(SharedTextureFallback::NoSelectedGpu));
        assert_eq!(
            fallback(Some(&descriptor(None, false)), &[DISCRETE]),
            Some(SharedTextureFallback::MissingLuid)
        );
        assert_eq!(
            fallback(Some(&descriptor(Some(DISCRETE), false)), &[DISCRETE]),
            Some(SharedTextureFallback::MissingExternalMemory)
        );
        assert_eq!(
            fallback(Some(&descriptor(Some(DISCRETE), true)), &[INTEGRATED]),
            Some(SharedTextureFallback::NoMatchingAdapter)
        );
        assert_eq!(
            fallback(Some(&descriptor(Some(DISCRETE), true)), &[]),
            Some(SharedTextureFallback::NoMatchingAdapter)
        );
        let names = [
            SharedTextureFallback::NoSelectedGpu,
            SharedTextureFallback::MissingLuid,
            SharedTextureFallback::MissingExternalMemory,
            SharedTextureFallback::NoMatchingAdapter,
        ]
        .map(SharedTextureFallback::name);
        assert!(names.iter().enumerate().all(|(index, name)| !names[..index].contains(name)));
    }
}
