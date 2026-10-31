// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Discover compatible host GPUs and bind initialization to the caller's exact selection.
use crate::GpuError;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};

const MAX_ADAPTERS: usize = 32;
const DISCOVERY_BYTES: u64 = 512 * 1024;
const DEVICE_BYTES: u64 = 8 * 1024 * 1024;
static NEXT_CATALOG: AtomicU64 = AtomicU64::new(1);

/// Process-local identity in exactly one immutable discovery result.
/// IDs cannot select devices from another catalog or be persisted across restarts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterId {
    catalog: u64,
    index: u8,
}
impl AdapterId {
    /// Identity outside every live catalog, for descriptor policy tests; it selects no device.
    #[cfg(feature = "boundary-fixtures")]
    #[doc(hidden)]
    pub const fn fixture() -> Self {
        Self { catalog: 0, index: 0 }
    }
}
/// Native graphics API used by the selected device.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Backend {
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
}
/// Driver-reported hardware class, independent of compatibility and selection policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterClass {
    /// A GPU sharing system memory.
    Integrated,
    /// A GPU with dedicated local memory.
    Discrete,
    /// A virtual GPU.
    Virtual,
    /// A software device; displayed in discovery but rejected for GPU execution.
    Software,
    /// A device whose hardware class was not reported.
    Other,
}
/// One bounded host descriptor; the application owns its presentation and startup choice.
#[derive(Clone, Debug)]
pub struct AdapterDescriptor {
    /// Selection identity, valid only while its catalog remains owned.
    pub id: AdapterId,
    /// Driver-reported display name.
    pub name: String,
    /// Driver-reported vendor identifier.
    pub vendor: u32,
    /// Driver-reported device identifier.
    pub device: u32,
    /// Native graphics API.
    pub backend: Backend,
    /// Driver-reported hardware class.
    pub class: AdapterClass,
    /// Whether storage buffers and workgroups meet the implemented image-kernel requirements.
    pub compatible: bool,
    /// Operating-system adapter identity, when the driver reports a valid LUID.
    pub luid: Option<crate::AdapterLuid>,
    /// Whether the device can import composition-owned D3D11 textures as display targets.
    /// This requires Vulkan `VK_KHR_external_memory_win32` and a valid LUID.
    pub shared_textures: bool,
}
struct Entry {
    descriptor: AdapterDescriptor,
    adapter: wgpu::Adapter,
}
/// Immutable enumerated devices with original discovery custody.
/// Discovery never substitutes a power-preference result for an explicit selection.
pub struct AdapterCatalog {
    instance: wgpu::Instance,
    identity: u64,
    entries: Vec<Entry>,
    _memory: MemoryClaim,
}
impl AdapterCatalog {
    /// Enumerate native Vulkan/Metal adapters inside an admitted async job.
    /// Application-owned inventory is bounded to 32 devices and 512 KiB; driver internals are opaque.
    /// # Errors
    /// Preserves original admission/control errors and rejects an excessive inventory.
    pub async fn discover(context: &JobContext) -> Result<Self, GpuError> {
        check(context)?;
        let memory = context
            .memory()
            .claim(DISCOVERY_BYTES)
            .map_err(|cause| GpuError::new("gpu_discovery_memory", cause))?;
        let identity = NEXT_CATALOG
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| value.checked_add(1))
            .map_err(|_| GpuError::invalid("GPU discovery identity exhausted"))?;
        let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapters = instance.enumerate_adapters(backends).await;
        if adapters.len() > MAX_ADAPTERS {
            return Err(GpuError::invalid("GPU adapter inventory exceeds its bound"));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(adapters.len())
            .map_err(|cause| GpuError::new("gpu_discovery_inventory", cause))?;
        for adapter in adapters {
            check(context)?;
            let info = adapter.get_info();
            let backend = match info.backend {
                wgpu::Backend::Vulkan => Backend::Vulkan,
                wgpu::Backend::Metal => Backend::Metal,
                _ => {
                    return Err(GpuError::invalid("Discovery returned an unrequested GPU backend"));
                }
            };
            if info.name.len() > 4096 {
                return Err(GpuError::invalid("GPU adapter name exceeds its bound"));
            }
            let class = class(info.device_type);
            let compatible = compatible(class, &adapter.limits());
            #[cfg(windows)]
            let luid = crate::shared::adapter_luid(&adapter);
            #[cfg(not(windows))]
            let luid = None;
            let shared_textures = backend == Backend::Vulkan
                && luid.is_some()
                && adapter.features().contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_WIN32);
            entries.push(Entry {
                descriptor: AdapterDescriptor {
                    id: AdapterId { catalog: identity, index: entries.len() as u8 },
                    name: info.name,
                    vendor: info.vendor,
                    device: info.device,
                    backend,
                    class,
                    compatible,
                    luid,
                    shared_textures,
                },
                adapter,
            });
        }
        Ok(Self { instance, identity, entries, _memory: memory })
    }
    /// Borrow discovery metadata without exposing raw device construction or queue handles.
    pub fn adapters(&self) -> impl ExactSizeIterator<Item = &AdapterDescriptor> {
        self.entries.iter().map(|entry| &entry.descriptor)
    }
    /// Initialize exactly one selected compatible adapter inside an admitted async job.
    /// The device retains the original renderer admission, with an eight-MiB opaque-driver allowance.
    /// # Errors
    /// Rejects foreign/stale identities and unsupported devices; preserves native initialization errors.
    pub async fn select(
        mut self,
        id: AdapterId,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Arc<GpuDevice>, GpuError> {
        if id.catalog != self.identity || usize::from(id.index) >= self.entries.len() {
            return Err(GpuError::invalid("GPU selection belongs to a different discovery result"));
        }
        let entry = self.entries.swap_remove(usize::from(id.index));
        if !entry.descriptor.compatible {
            return Err(GpuError::invalid("Selected GPU does not support the image kernels"));
        }
        check(context)?;
        let claim = memory
            .claim(DEVICE_BYTES)
            .map_err(|cause| GpuError::new("gpu_device_memory", cause))?;
        let (device, queue) = entry
            .adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("SugarSubstitute image DAG"),
                required_features: if entry.descriptor.shared_textures {
                    wgpu::Features::VULKAN_EXTERNAL_MEMORY_WIN32
                } else {
                    wgpu::Features::empty()
                },
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await
            .map_err(|cause| GpuError::new("gpu_device_initialization", cause))?;
        tracing::info!(operation="image_gpu_selection", adapter=%entry.descriptor.name,
            vendor=entry.descriptor.vendor, device=entry.descriptor.device,
            backend=?entry.descriptor.backend, class=?entry.descriptor.class,
            shared_textures=entry.descriptor.shared_textures,
            "Initialized the explicitly selected image GPU");
        let faults = Arc::new(crate::fault::Faults::default());
        let callback = faults.clone();
        device.on_uncaptured_error(Arc::new(move |cause| callback.record(cause)));
        let callback = faults.clone();
        device.set_device_lost_callback(move |reason, message| callback.lost(reason, message));
        let timings = crate::timing::GpuDiagnostics::new(
            entry.descriptor.vendor,
            entry.descriptor.device,
            memory,
        )?;
        Ok(Arc::new(GpuDevice {
            descriptor: entry.descriptor,
            instance: self.instance,
            adapter: entry.adapter,
            device,
            queue,
            memory: memory.clone(),
            faults,
            submission: Arc::new(crate::coordination::SubmissionGate::default()),
            timings,
            _memory: claim,
        }))
    }
}
/// One initialized backend/device for the renderer's whole startup lifetime.
/// Application memory claims and submitted-resource lifetimes remain with this renderer adapter.
pub struct GpuDevice {
    pub(crate) descriptor: AdapterDescriptor,
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) memory: MemoryLease,
    pub(crate) faults: Arc<crate::fault::Faults>,
    pub(crate) submission: Arc<crate::coordination::SubmissionGate>,
    pub(crate) timings: crate::timing::GpuDiagnostics,
    _memory: MemoryClaim,
}
impl GpuDevice {
    /// Actual selected hardware metadata, captured from the adapter used for initialization.
    pub fn descriptor(&self) -> &AdapterDescriptor {
        &self.descriptor
    }
    /// Clone observations without retaining this device; the UI can report active native calls
    /// even while a preparation worker is blocked. Reporting never polls or waits for the GPU.
    pub fn diagnostics(&self) -> crate::timing::GpuDiagnostics {
        self.timings.clone()
    }
    pub(crate) fn submission(
        &self,
        context: &JobContext,
    ) -> Result<crate::coordination::SubmissionPermit, GpuError> {
        let _timing = self.timings.begin(crate::timing::Phase::Coordination, 0, 1);
        self.submission.submission(context)
    }
}
fn class(value: wgpu::DeviceType) -> AdapterClass {
    match value {
        wgpu::DeviceType::IntegratedGpu => AdapterClass::Integrated,
        wgpu::DeviceType::DiscreteGpu => AdapterClass::Discrete,
        wgpu::DeviceType::VirtualGpu => AdapterClass::Virtual,
        wgpu::DeviceType::Cpu => AdapterClass::Software,
        _ => AdapterClass::Other,
    }
}
fn compatible(class: AdapterClass, limits: &wgpu::Limits) -> bool {
    class != AdapterClass::Software
        && wgpu::Limits::default().check_limits(limits)
        && limits.max_storage_buffers_per_shader_stage >= 8
        && limits.max_storage_buffer_binding_size >= 4 * 1024 * 1024
        && limits.max_compute_invocations_per_workgroup >= 64
}

pub(crate) fn check(context: &JobContext) -> Result<(), GpuError> {
    if context.cancellation_requested() {
        return Err(GpuError::new("gpu_control", sugarsubstitute_image_ops::ImageError::Cancelled));
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(GpuError::new("gpu_control", sugarsubstitute_image_ops::ImageError::Deadline));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integrated_and_discrete_devices_share_the_same_kernel_requirements() {
        let limits = wgpu::Limits::default();
        assert!(compatible(AdapterClass::Integrated, &limits));
        assert!(compatible(AdapterClass::Discrete, &limits));
        assert!(!compatible(AdapterClass::Software, &limits));
    }
    #[test]
    fn insufficient_storage_bindings_or_workgroups_reject_the_device() {
        let limits = wgpu::Limits { max_storage_buffers_per_shader_stage: 7, ..Default::default() };
        assert!(!compatible(AdapterClass::Integrated, &limits));
        let limits =
            wgpu::Limits { max_compute_invocations_per_workgroup: 32, ..Default::default() };
        assert!(!compatible(AdapterClass::Discrete, &limits));
    }
    fn descriptor(index: u8, vendor: u32, device: u32) -> AdapterDescriptor {
        AdapterDescriptor {
            id: AdapterId { catalog: 1, index },
            name: "Qualified GPU".into(),
            vendor,
            device,
            backend: Backend::Vulkan,
            class: AdapterClass::Discrete,
            compatible: true,
            luid: None,
            shared_textures: false,
        }
    }
    #[test]
    fn explicit_model_selection_is_independent_of_native_enumeration_order() -> Result<(), GpuError>
    {
        let integrated = descriptor(0, 0x8086, 0xa780);
        let discrete = descriptor(1, 0x10de, 0x2b85);
        let key = "vulkan:8086:a780".parse()?;
        assert_eq!(
            crate::selection::resolve([&discrete, &integrated].into_iter(), key)?,
            integrated.id
        );
        assert_eq!(
            crate::selection::resolve([&integrated, &discrete].into_iter(), key)?,
            integrated.id
        );
        assert_eq!(key.to_string(), "vulkan:8086:a780");
        Ok(())
    }
    #[test]
    fn absent_ambiguous_and_malformed_selectors_cannot_choose_an_arbitrary_gpu()
    -> Result<(), GpuError> {
        let first = descriptor(0, 0x10de, 0x2b85);
        let second = descriptor(1, 0x10de, 0x2b85);
        assert!(crate::selection::resolve([&first, &second].into_iter(), first.key()).is_err());
        assert!(
            crate::selection::resolve([&first].into_iter(), "metal:106b:0000".parse()?).is_err()
        );
        for value in [
            "dx12:10de:2b85",
            "vulkan:0x10de:2b85",
            "vulkan:10de",
            "vulkan::2b85",
            "vulkan:+10de:2b85",
            "vulkan:10de:2b85:0",
        ] {
            assert!(value.parse::<crate::AdapterKey>().is_err());
        }
        Ok(())
    }
}
