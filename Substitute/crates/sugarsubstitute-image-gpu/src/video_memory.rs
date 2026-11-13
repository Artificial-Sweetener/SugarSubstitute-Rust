// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read how much device-local video memory the driver grants this process.
//!
//! The query reads `VK_EXT_memory_budget` through the selected Vulkan physical device. It is a
//! nonblocking property read without external synchronization requirements, so any thread may
//! call it while the device renders.

use crate::GpuDevice;
use ash::vk;
use wgpu::hal::api::Vulkan;

/// Device-local video memory this process may use and currently uses, in bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VideoMemoryBudget {
    /// Memory the driver estimates this process can allocate without oversubscription.
    pub budget_bytes: u64,
    /// Memory this process currently has allocated.
    pub usage_bytes: u64,
}

impl GpuDevice {
    /// The device-local budget and usage, or `None` when the driver lacks `VK_EXT_memory_budget`.
    pub fn video_memory_budget(&self) -> Option<VideoMemoryBudget> {
        // SAFETY: The guard borrows the live adapter only for read-only property queries.
        let hal = unsafe { self.adapter.as_hal::<Vulkan>() }?;
        if !hal.physical_device_capabilities().supports_extension(ash::ext::memory_budget::NAME) {
            return None;
        }
        let instance = hal.shared_instance();
        let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
        let local_heaps: Vec<usize> = {
            let mut properties =
                vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
            // SAFETY: The device supports the extension whose structure is chained; every chained
            // structure outlives the call, which only writes into them.
            unsafe {
                instance.raw_instance().get_physical_device_memory_properties2(
                    hal.raw_physical_device(),
                    &mut properties,
                );
            }
            let heaps = properties.memory_properties;
            (0..heaps.memory_heap_count as usize)
                .filter(|&heap| {
                    heaps.memory_heaps[heap].flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL)
                })
                .collect()
        };
        Some(local_heaps.into_iter().fold(VideoMemoryBudget::default(), |total, heap| {
            VideoMemoryBudget {
                budget_bytes: total.budget_bytes.saturating_add(budget.heap_budget[heap]),
                usage_bytes: total.usage_bytes.saturating_add(budget.heap_usage[heap]),
            }
        }))
    }
}
