// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the native canvas drawing device on an admitted worker.

use super::composition_adapter::{CompositionAdapter, SharedTextureFallback, composition_adapter};
use std::{
    error::Error,
    ffi::c_void,
    fmt, io,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_gpu::{AdapterDescriptor, AdapterLuid};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HMODULE},
        Graphics::{
            Direct2D::{
                D2D1_FACTORY_TYPE, D2D1_FACTORY_TYPE_SINGLE_THREADED, ID2D1Device, ID2D1Factory1,
            },
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_FENCE_FLAG_NONE, D3D11_SDK_VERSION,
                D3D11CreateDevice, ID3D11Device, ID3D11Device5, ID3D11DeviceContext4, ID3D11Fence,
            },
            Dxgi::{
                CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIAdapter,
                IDXGIAdapter1, IDXGIDevice, IDXGIFactory1,
            },
        },
    },
    core::{GUID, HRESULT, Interface},
};

windows::core::link!("d2d1.dll" "system" fn D2D1CreateFactory(
    factory_type: D2D1_FACTORY_TYPE,
    iid: *const GUID,
    options: *const c_void,
    result: *mut *mut c_void,
) -> HRESULT);

const DEVICE_BYTES: u64 = 8 * 1024 * 1024;

/// A worker-prepared drawing device and its original admitted canvas reservation.
/// The desktop adapter uses it only after the preparing job has actually completed.
/// The single-threaded Direct2D factory and the D3D11 immediate context are used only on the
/// UI thread after preparation; workers use only the free-threaded device and its read fence.
pub struct CanvasGraphics {
    native: Option<NativeGraphics>,
    placement: CompositionAdapter,
    memory: MemoryLease,
    _claim: MemoryClaim,
}

/// Drop drawing resources before their factory and underlying devices.
struct NativeGraphics {
    drawing: ID2D1Device,
    reads: Option<ReadFence>,
    _factory: ID2D1Factory1,
    _dxgi: IDXGIDevice,
    d3d: ID3D11Device,
}

/// Monotonic D3D11 fence ordering composition reads of shared textures before later GPU draws.
pub(super) struct ReadFence {
    pub(super) fence: ID3D11Fence,
    next: AtomicU64,
}

/// The original operation and cause from a bounded graphics preparation job.
#[derive(Debug)]
pub struct CanvasGraphicsError {
    operation: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}

impl CanvasGraphicsError {
    /// Preserve the native or admission cause with its operation identity.
    pub(super) fn new(operation: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { operation, cause: Box::new(cause) }
    }
}

impl fmt::Display for CanvasGraphicsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.cause)
    }
}

impl Error for CanvasGraphicsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl CanvasGraphics {
    /// Create graphics objects on an admitted blocking worker, without touching a window.
    ///
    /// `selected` is the canvas session's initialized GPU, if any. When it can share textures,
    /// the device is created on the hardware adapter with the same LUID even when the system's
    /// default adapter differs; otherwise the default hardware adapter is used and the named
    /// fallback is logged and retained. The supplied lease must be the canvas's retained
    /// reservation. Native calls may outlive a cancellation request; the caller must observe
    /// the worker's actual terminal result.
    /// # Errors
    /// Preserves allocation, cancellation, deadline and native device-creation failures.
    pub fn prepare(
        memory: &MemoryLease,
        context: &JobContext,
        selected: Option<&AdapterDescriptor>,
    ) -> Result<Arc<Self>, CanvasGraphicsError> {
        checkpoint(context)?;
        let claim = memory
            .claim(DEVICE_BYTES)
            .map_err(|cause| CanvasGraphicsError::new("canvas_device_budget", cause))?;
        let hardware = hardware_adapters()?;
        let luids: Vec<_> = hardware.iter().map(|(luid, _)| *luid).collect();
        let placement = composition_adapter(selected, &luids);
        let adapter = match placement {
            CompositionAdapter::Shared(luid) => {
                tracing::info!(
                    operation = "canvas_shared_texture_adapter",
                    luid_low = luid.low(),
                    luid_high = luid.high(),
                    "Placing canvas composition on the selected GPU adapter"
                );
                let adapter = hardware
                    .into_iter()
                    .find_map(|(candidate, adapter)| (candidate == luid).then_some(adapter))
                    .ok_or_else(|| missing("canvas_shared_adapter"))?;
                Some(
                    adapter
                        .cast::<IDXGIAdapter>()
                        .map_err(|cause| CanvasGraphicsError::new("query_canvas_adapter", cause))?,
                )
            }
            CompositionAdapter::Default(reason) => {
                log_fallback(reason, selected);
                None
            }
        };
        checkpoint(context)?;
        // An explicit adapter requires the unknown driver type; the default uses hardware.
        let driver =
            if adapter.is_some() { D3D_DRIVER_TYPE_UNKNOWN } else { D3D_DRIVER_TYPE_HARDWARE };
        let mut d3d = None;
        // SAFETY: The worker owns the output slot, including any partially returned interface.
        unsafe {
            D3D11CreateDevice(
                adapter.as_ref(),
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
        }
        .map_err(|cause| CanvasGraphicsError::new("create_canvas_d3d", cause))?;
        let d3d = d3d.ok_or_else(|| missing("canvas_d3d_device"))?;
        checkpoint(context)?;
        let dxgi: IDXGIDevice =
            d3d.cast().map_err(|cause| CanvasGraphicsError::new("query_canvas_dxgi", cause))?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: The requested IID and writable output belong to this worker operation.
        let status = unsafe {
            D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                &ID2D1Factory1::IID,
                std::ptr::null(),
                &mut raw,
            )
        };
        // SAFETY: The returned interface, including a partial failure, is owned by this worker.
        let factory: ID2D1Factory1 = unsafe { adopt(raw, status, "create_canvas_d2d_factory")? };
        checkpoint(context)?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: The retained factory and DXGI device remain live through this output transfer.
        let status =
            unsafe { (factory.vtable().CreateDevice)(factory.as_raw(), dxgi.as_raw(), &mut raw) };
        // SAFETY: CreateDevice transfers the requested drawing-device reference.
        let drawing: ID2D1Device = unsafe { adopt(raw, status, "create_canvas_d2d_device")? };
        let reads = match placement {
            CompositionAdapter::Shared(_) => {
                let device: ID3D11Device5 = d3d.cast().map_err(|cause| {
                    CanvasGraphicsError::new("query_canvas_fence_device", cause)
                })?;
                let mut fence: Option<ID3D11Fence> = None;
                // SAFETY: The free-threaded device writes one fence into this worker-owned slot.
                unsafe { device.CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut fence) }
                    .map_err(|cause| CanvasGraphicsError::new("create_canvas_read_fence", cause))?;
                let fence = fence.ok_or_else(|| missing("canvas_read_fence"))?;
                Some(ReadFence { fence, next: AtomicU64::new(0) })
            }
            CompositionAdapter::Default(_) => None,
        };
        checkpoint(context)?;
        Ok(Arc::new(Self {
            native: Some(NativeGraphics { d3d, _dxgi: dxgi, _factory: factory, drawing, reads }),
            placement,
            memory: memory.clone(),
            _claim: claim,
        }))
    }

    /// Supply a deterministic device boundary to application tests without starting a GPU.
    ///
    /// # Errors
    /// Preserves reservation, cancellation and deadline failures.
    #[cfg(any(test, feature = "boundary-fixtures"))]
    #[doc(hidden)]
    pub fn fixture(
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Arc<Self>, CanvasGraphicsError> {
        checkpoint(context)?;
        let claim = memory
            .claim(DEVICE_BYTES)
            .map_err(|cause| CanvasGraphicsError::new("canvas_device_budget", cause))?;
        Ok(Arc::new(Self {
            native: None,
            placement: CompositionAdapter::Default(SharedTextureFallback::NoSelectedGpu),
            memory: memory.clone(),
            _claim: claim,
        }))
    }

    /// Adapter whose Vulkan device may draw into this device's shared textures, if any.
    pub fn shared_textures(&self) -> Option<AdapterLuid> {
        match self.placement {
            CompositionAdapter::Shared(luid) if self.read_fence().is_some() => Some(luid),
            _ => None,
        }
    }

    /// Reason this device presents CPU frames only, when shared textures are unavailable.
    pub fn shared_texture_fallback(&self) -> Option<SharedTextureFallback> {
        match self.placement {
            CompositionAdapter::Default(reason) => Some(reason),
            CompositionAdapter::Shared(_) => None,
        }
    }

    /// Require the same retained reservation as the frame's canvas session.
    pub(crate) fn owns_memory(&self, memory: &MemoryLease) -> bool {
        self.memory.same_reservation(memory)
    }

    /// Borrow the retained reservation that funds shared textures.
    pub(super) fn memory(&self) -> &MemoryLease {
        &self.memory
    }

    /// Borrow the device only after worker preparation has completed.
    pub(crate) fn drawing(&self) -> Option<&ID2D1Device> {
        self.native.as_ref().map(|native| &native.drawing)
    }

    /// Borrow the free-threaded D3D11 device for resource creation.
    pub(crate) fn d3d(&self) -> Option<&ID3D11Device> {
        self.native.as_ref().map(|native| &native.d3d)
    }

    /// Borrow the shared-texture read fence when this device shares textures.
    pub(super) fn read_fence(&self) -> Option<&ReadFence> {
        self.native.as_ref().and_then(|native| native.reads.as_ref())
    }

    /// Queue a read-fence signal after every UI-thread read already queued and return its value.
    /// Call only on the UI thread, which owns the immediate context.
    pub(crate) fn signal_reads(&self) -> Result<u64, CanvasGraphicsError> {
        let native = self.native.as_ref().ok_or_else(|| missing("canvas_d3d_device"))?;
        let reads = native.reads.as_ref().ok_or_else(|| missing("canvas_read_fence"))?;
        let value = reads
            .next
            .fetch_add(1, Ordering::AcqRel)
            .checked_add(1)
            .ok_or_else(|| missing("canvas_read_fence_value"))?;
        // SAFETY: This UI-thread caller owns the immediate context; the retained fence outlives
        // the queued signal.
        unsafe {
            let immediate = native
                .d3d
                .GetImmediateContext()
                .map_err(|cause| CanvasGraphicsError::new("canvas_immediate_context", cause))?;
            let context: ID3D11DeviceContext4 = immediate
                .cast()
                .map_err(|cause| CanvasGraphicsError::new("query_canvas_context", cause))?;
            context
                .Signal(&reads.fence, value)
                .map_err(|cause| CanvasGraphicsError::new("signal_canvas_reads", cause))?;
            immediate.Flush();
        }
        Ok(value)
    }
}

/// Enumerate hardware DXGI adapters with their LUIDs; software adapters cannot share textures.
fn hardware_adapters() -> Result<Vec<(AdapterLuid, IDXGIAdapter1)>, CanvasGraphicsError> {
    // SAFETY: The factory is created and used only by this worker for enumeration.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(|cause| CanvasGraphicsError::new("create_canvas_dxgi_factory", cause))?;
    let mut adapters = Vec::new();
    for index in 0..64 {
        // SAFETY: Enumeration returns an owned adapter reference or DXGI_ERROR_NOT_FOUND.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(cause) if cause.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(cause) => return Err(CanvasGraphicsError::new("enumerate_canvas_adapters", cause)),
        };
        // SAFETY: The retained adapter writes its own descriptor.
        let description = unsafe { adapter.GetDesc1() }
            .map_err(|cause| CanvasGraphicsError::new("describe_canvas_adapter", cause))?;
        if description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let luid = description.AdapterLuid;
        adapters.push((AdapterLuid::from_parts(luid.LowPart, luid.HighPart), adapter));
    }
    Ok(adapters)
}

/// Record why shared GPU textures are unavailable for this composition device.
fn log_fallback(reason: SharedTextureFallback, selected: Option<&AdapterDescriptor>) {
    let adapter = selected.map_or("", |descriptor| descriptor.name.as_str());
    if reason == SharedTextureFallback::NoSelectedGpu {
        tracing::info!(
            operation = "canvas_shared_texture_fallback",
            reason = reason.name(),
            "Canvas composition presents CPU frames because no GPU renderer is selected"
        );
    } else {
        tracing::warn!(
            operation = "canvas_shared_texture_fallback",
            reason = reason.name(),
            adapter,
            "Canvas composition cannot share textures with the selected GPU; presenting CPU frames"
        );
    }
}

/// Observe cancellation and the original deadline before and after blocking native calls.
pub(super) fn checkpoint(context: &JobContext) -> Result<(), CanvasGraphicsError> {
    if context.cancellation_requested() {
        return Err(CanvasGraphicsError::new(
            "prepare_canvas_graphics",
            io::Error::from(io::ErrorKind::Interrupted),
        ));
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(CanvasGraphicsError::new(
            "prepare_canvas_graphics",
            io::Error::from(io::ErrorKind::TimedOut),
        ));
    }
    Ok(())
}

/// Describe a native call that succeeded without returning its required interface.
pub(super) fn missing(operation: &'static str) -> CanvasGraphicsError {
    CanvasGraphicsError::new(operation, windows::core::Error::from(E_POINTER))
}

/// Adopt a transferred native reference before interpreting the producing call's status.
unsafe fn adopt<T: Interface>(
    raw: *mut c_void,
    status: HRESULT,
    operation: &'static str,
) -> Result<T, CanvasGraphicsError> {
    // SAFETY: The caller requested T and transferred this output, including on failure.
    let retained = (!raw.is_null()).then(|| unsafe { T::from_raw(raw) });
    status.ok().map_err(|cause| CanvasGraphicsError::new(operation, cause))?;
    retained.ok_or_else(|| missing(operation))
}
