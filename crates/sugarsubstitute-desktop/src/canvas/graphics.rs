// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the native canvas drawing device on an admitted worker.

use std::{error::Error, ffi::c_void, fmt, io, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HMODULE},
        Graphics::{
            Direct2D::{
                D2D1_FACTORY_TYPE, D2D1_FACTORY_TYPE_SINGLE_THREADED, ID2D1Device, ID2D1Factory1,
            },
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device,
            },
            Dxgi::{IDXGIAdapter, IDXGIDevice},
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
/// Its single-threaded Direct2D factory is accessed serially: preparation ends on the
/// worker before the desktop begins using it, and retirement waits for desktop custody.
pub struct CanvasGraphics {
    native: Option<NativeGraphics>,
    memory: MemoryLease,
    _claim: MemoryClaim,
}

/// Drop drawing resources before their factory and underlying devices.
struct NativeGraphics {
    drawing: ID2D1Device,
    _factory: ID2D1Factory1,
    _dxgi: IDXGIDevice,
    _d3d: ID3D11Device,
}

/// The original operation and cause from a bounded graphics preparation job.
#[derive(Debug)]
pub struct CanvasGraphicsError {
    operation: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}

impl CanvasGraphicsError {
    /// Preserve the native or admission cause with its operation identity.
    fn new(operation: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
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
    /// The supplied lease must be the canvas's retained reservation. Native calls may outlive
    /// a cancellation request; the caller must observe the worker's actual terminal result.
    /// # Errors
    /// Preserves allocation, cancellation, deadline and native device-creation failures.
    pub fn prepare(
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Arc<Self>, CanvasGraphicsError> {
        checkpoint(context)?;
        let claim = memory
            .claim(DEVICE_BYTES)
            .map_err(|cause| CanvasGraphicsError::new("canvas_device_budget", cause))?;
        let mut d3d = None;
        // SAFETY: The worker owns the output slot, including any partially returned interface.
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_HARDWARE,
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
        checkpoint(context)?;
        Ok(Arc::new(Self {
            native: Some(NativeGraphics { _d3d: d3d, _dxgi: dxgi, _factory: factory, drawing }),
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
        Ok(Arc::new(Self { native: None, memory: memory.clone(), _claim: claim }))
    }

    /// Require the same retained reservation as the frame's canvas session.
    pub(crate) fn owns_memory(&self, memory: &MemoryLease) -> bool {
        self.memory.same_reservation(memory)
    }

    /// Borrow the device only after worker preparation has completed.
    pub(crate) fn drawing(&self) -> Option<&ID2D1Device> {
        self.native.as_ref().map(|native| &native.drawing)
    }
}

/// Observe cancellation and the original deadline before and after blocking native calls.
fn checkpoint(context: &JobContext) -> Result<(), CanvasGraphicsError> {
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
fn missing(operation: &'static str) -> CanvasGraphicsError {
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
