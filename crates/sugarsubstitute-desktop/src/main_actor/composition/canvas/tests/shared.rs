// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify Vulkan drawing into shared textures and their composition copy on every GPU.

use super::*;
use crate::canvas::{CanvasGraphics, SharedCanvasTexture, SharedFrame};
use std::{error::Error, io, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_gpu::{
    AdapterCatalog, AdapterClass, AdapterDescriptor, Backend, DisplayImage, GpuDevice,
    SharedSurface, SharedTarget,
};
use windows::Win32::{
    Foundation::S_OK,
    Graphics::Direct3D11::{
        D3D11_QUERY_DATA_TIMESTAMP_DISJOINT, D3D11_QUERY_DESC, D3D11_QUERY_TIMESTAMP,
        D3D11_QUERY_TIMESTAMP_DISJOINT, ID3D11Asynchronous, ID3D11DeviceContext, ID3D11Query,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const BUDGET: u64 = 512 * 1024 * 1024;

fn io(cause: impl std::fmt::Display) -> io::Error {
    io::Error::other(cause.to_string())
}

/// Borrow the worker-side texture allocator of a composition device that shares textures.
fn factory(graphics: &CanvasGraphics) -> Result<crate::canvas::SharedTextureFactory, io::Error> {
    graphics.shared_texture_factory().ok_or_else(|| io("Composition device shares no textures"))
}

/// Deterministic opaque pattern whose every channel varies across both axes.
fn pattern(extent: Extent) -> Vec<u32> {
    (0..extent.height())
        .flat_map(|y| {
            (0..extent.width()).map(move |x| {
                0xff00_0000
                    | ((x * 7 + y) & 255) << 16
                    | ((y * 5 + x * 3) & 255) << 8
                    | (x ^ y) & 255
            })
        })
        .collect()
}

/// GPU timestamp interval around UI-thread composition work on the immediate context.
struct GpuInterval {
    context: ID3D11DeviceContext,
    disjoint: ID3D11Query,
    start: ID3D11Query,
    end: ID3D11Query,
}

impl GpuInterval {
    fn begin(graphics: &CanvasGraphics) -> TestResult<Self> {
        let device = graphics.d3d().ok_or("Composition device absent")?;
        let query = |kind| -> TestResult<ID3D11Query> {
            let mut query = None;
            // SAFETY: The test thread owns this device query output slot.
            unsafe {
                device
                    .CreateQuery(&D3D11_QUERY_DESC { Query: kind, MiscFlags: 0 }, Some(&mut query))
            }?;
            Ok(query.ok_or("Timestamp query absent")?)
        };
        let interval = Self {
            // SAFETY: The immediate context is used only on this test thread.
            context: unsafe { device.GetImmediateContext() }?,
            disjoint: query(D3D11_QUERY_TIMESTAMP_DISJOINT)?,
            start: query(D3D11_QUERY_TIMESTAMP)?,
            end: query(D3D11_QUERY_TIMESTAMP)?,
        };
        // SAFETY: These queries belong to this context's device.
        unsafe {
            interval.context.Begin(&interval.disjoint);
            interval.context.End(&interval.start);
        }
        Ok(interval)
    }

    /// End the interval and spin on the non-blocking query results; no sleep is involved.
    fn finish(self) -> TestResult<f64> {
        // SAFETY: These queries belong to this context's device.
        unsafe {
            self.context.End(&self.end);
            self.context.End(&self.disjoint);
            self.context.Flush();
        }
        let mut disjoint = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
        let (mut start, mut end) = (0_u64, 0_u64);
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        for (query, data, size) in [
            (&self.disjoint, (&raw mut disjoint).cast(), size_of_val(&disjoint)),
            (&self.start, (&raw mut start).cast(), size_of::<u64>()),
            (&self.end, (&raw mut end).cast(), size_of::<u64>()),
        ] {
            let query: ID3D11Asynchronous = query.cast()?;
            loop {
                // SAFETY: `data` addresses a live result of exactly `size` bytes.
                let status = unsafe {
                    (self.context.vtable().GetData)(
                        self.context.as_raw(),
                        query.as_raw(),
                        data,
                        u32::try_from(size)?,
                        0,
                    )
                };
                if status == S_OK {
                    break;
                }
                status.ok()?;
                if Instant::now() >= deadline {
                    return Err("GPU timestamp query did not complete".into());
                }
                std::hint::spin_loop();
            }
        }
        if disjoint.Disjoint.as_bool() || disjoint.Frequency == 0 {
            return Err("GPU timestamps were disjoint".into());
        }
        #[allow(clippy::cast_precision_loss)]
        Ok(end.saturating_sub(start) as f64 * 1_000_000.0 / disjoint.Frequency as f64)
    }
}

/// Measured composition interop costs for one adapter and frame extent.
struct Timing {
    vulkan_draw_microseconds: f64,
    copy_gpu_microseconds: f64,
    handoff_microseconds: f64,
}

/// Initialize exactly the enumerated adapter on a fresh catalog, matching it by model and LUID.
fn select(
    execution: &mut harness::BlockingExecution,
    memory: &MemoryLease,
    descriptor: &AdapterDescriptor,
) -> TestResult<Arc<GpuDevice>> {
    let (memory, key, luid) = (memory.clone(), descriptor.key(), descriptor.luid);
    Ok(execution
        .asynchronous(2 * 1024 * 1024, move |context| async move {
            let catalog = AdapterCatalog::discover(&context).await.map_err(io)?;
            let id = catalog
                .adapters()
                .find(|adapter| adapter.key() == key && adapter.luid == luid)
                .map(|adapter| adapter.id)
                .ok_or_else(|| io("Enumerated adapter disappeared"))?;
            catalog.select(id, &memory, &context).await.map_err(io)
        })
        .map_err(io)?)
}

/// Draw one pattern through Vulkan, copy it into a composition surface and read it back.
fn qualify_extent(
    execution: &mut harness::BlockingExecution,
    composition: &IDCompositionDevice,
    memory: &MemoryLease,
    graphics: &Arc<CanvasGraphics>,
    device: &Arc<GpuDevice>,
    descriptor: &AdapterDescriptor,
    extent: Extent,
) -> TestResult<Timing> {
    let texture = {
        let graphics = graphics.clone();
        Arc::new(
            execution
                .blocking(64 * 1024, move |context| {
                    SharedCanvasTexture::create(&factory(&graphics)?, extent, &context).map_err(io)
                })
                .map_err(io)?,
        )
    };
    let pixels = Arc::new(pattern(extent));
    let (drawn_pixels, drawn_texture, drawn_device) =
        (pixels.clone(), texture.clone(), device.clone());
    let (image, target, surface, vulkan) = execution
        .blocking(64 * 1024 * 1024, move |context| {
            let image =
                DisplayImage::from_pixels(drawn_device.clone(), extent, &drawn_pixels, &context)
                    .map_err(io)?;
            let handle = drawn_texture.handle();
            let target = SharedTarget::import(&drawn_device, &handle, &context).map_err(io)?;
            let mut surface = SharedSurface::new(drawn_device).map_err(io)?;
            drawn_texture.await_reads(&context).map_err(io)?;
            // The first draw compiles the display pipeline; the second measures steady state.
            surface.draw(&target, &image, &[], &context).map_err(io)?;
            let started = Instant::now();
            surface.draw(&target, &image, &[], &context).map_err(io)?;
            Ok::<_, io::Error>((image, target, surface, started.elapsed()))
        })
        .map_err(io)?;
    // SAFETY: This test retains the device and its unbound root on the creating thread.
    let root = unsafe { composition.CreateVisual()? };
    let mut canvas = Canvas::new(composition.clone(), root, memory.clone(), graphics.clone())?;
    let viewport = PhysicalRect { x: 0, y: 0, width: extent.width(), height: extent.height() };
    let placement = CanvasPlacement { viewport, clip: viewport };
    let stamp = |generation| -> TestResult<FrameStamp> {
        Ok(FrameStamp { surface: SurfaceId::new(9)?, generation: Generation::new(generation)? })
    };
    let shared =
        |target| CanvasContent::Shared(SharedFrame { pending: true, target, texture: &texture });
    // The first commit allocates the composition surface; the second measures steady state.
    let first = stamp(1)?;
    assert_eq!(canvas.present(&shared(first), placement)?, Some(first));
    let timed = stamp(2)?;
    let interval = GpuInterval::begin(graphics)?;
    assert_eq!(canvas.present(&shared(timed), placement)?, Some(timed));
    let copy_gpu_microseconds = interval.finish()?;
    let handoff_texture = texture.clone();
    let handoff = execution
        .blocking(64 * 1024, move |context| {
            let started = Instant::now();
            handoff_texture.await_reads(&context).map_err(io)?;
            Ok::<_, io::Error>(started.elapsed())
        })
        .map_err(io)?;

    let mut capture = |content: &CanvasContent<'_>, expected: &[u32]| -> TestResult {
        canvas.capture_next = true;
        let target = content.target();
        assert_eq!(canvas.present(content, placement)?, Some(target));
        let captured = canvas
            .active
            .as_mut()
            .and_then(|allocation| allocation.capture.take())
            .ok_or("Composition capture absent")?;
        if let Some(index) = captured.iter().zip(expected).position(|(a, b)| a != b) {
            return Err(format!(
                "Composition pixel {index} differs: actual={:#010x}, expected={:#010x}",
                captured[index], expected[index]
            )
            .into());
        }
        assert_eq!(captured.len(), expected.len());
        assert!(canvas.retains(target, placement));
        Ok(())
    };
    capture(&shared(stamp(3)?), &pixels)?;
    // CPU-prepared previews and GPU frames alternate on the same retained surface.
    let cpu_pixels = vec![0xff20_4060; pixels.len()];
    let damage = DamagePlan::full(extent);
    let cpu = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp(4)?,
        base: None,
        damage: &damage,
        pixels: &cpu_pixels,
    };
    capture(&CanvasContent::Cpu(cpu), &cpu_pixels)?;
    capture(&shared(stamp(5)?), &pixels)?;
    assert!(canvas.active.as_ref().is_some_and(|allocation| allocation.extent() == extent));
    assert!(!canvas.has_retirements());

    // Regressed generations and textures exported by another composition device are rejected
    // before any native effect; the committed frame remains the acknowledged content.
    assert!(canvas.present(&shared(stamp(4)?), placement).is_err());
    assert!(canvas.retains(stamp(5)?, placement));
    let (foreign_memory, selected) = (memory.clone(), descriptor.clone());
    let foreign = execution.blocking(64 * 1024, move |context| {
        let graphics =
            CanvasGraphics::prepare(&foreign_memory, &context, Some(&selected)).map_err(io)?;
        SharedCanvasTexture::create(&factory(&graphics)?, extent, &context).map_err(io)
    });
    let foreign = foreign.map_err(io)?;
    let foreign_frame =
        CanvasContent::Shared(SharedFrame { pending: true, target: stamp(6)?, texture: &foreign });
    assert!(canvas.present(&foreign_frame, placement).is_err());
    assert!(canvas.retains(stamp(5)?, placement));
    drop(foreign);

    // Hiding withdraws the shared frame; completion follows the compositor's retirement.
    assert!(!canvas.hide()?);
    assert!(!canvas.retains(stamp(5)?, placement));
    execution.wait_composition(canvas.next_wait().ok_or("Hide wait absent")?).map_err(io)?;
    canvas.reap_retirements()?;
    assert!(canvas.hide()?);
    canvas.end_draws()?;
    canvas.release_after_detach()?;
    drop(canvas);
    // The application may still hold the texture after composition releases it; its recorded
    // reads remain awaitable before any further Vulkan draw.
    let held = texture.clone();
    execution
        .blocking(64 * 1024, move |context| held.await_reads(&context).map_err(io))
        .map_err(io)?;
    drop((surface, target, image));
    Ok(Timing {
        vulkan_draw_microseconds: vulkan.as_secs_f64() * 1_000_000.0,
        copy_gpu_microseconds,
        handoff_microseconds: handoff.as_secs_f64() * 1_000_000.0,
    })
}

/// Qualify one adapter end to end, reporting the measured interop costs.
fn qualify_adapter(
    execution: &mut harness::BlockingExecution,
    composition: &IDCompositionDevice,
    memory: &MemoryLease,
    descriptor: &AdapterDescriptor,
) -> TestResult {
    let luid = descriptor.luid.ok_or("Vulkan driver reports no adapter LUID")?;
    if !descriptor.shared_textures {
        return Err("Vulkan driver lacks VK_KHR_external_memory_win32 texture import".into());
    }
    let device = select(execution, memory, descriptor)?;
    let (graphics_memory, selected) = (memory.clone(), descriptor.clone());
    let graphics = execution
        .blocking(64 * 1024, move |context| {
            CanvasGraphics::prepare(&graphics_memory, &context, Some(&selected)).map_err(io)
        })
        .map_err(io)?;
    if graphics.shared_textures() != Some(luid) {
        return Err(format!(
            "Composition device did not match the Vulkan LUID: {:?}",
            graphics.shared_texture_fallback()
        )
        .into());
    }
    for (width, height) in [(64, 48), (1920, 1080), (3840, 2160)] {
        let extent = Extent::new(width, height)?;
        let timing =
            qualify_extent(execution, composition, memory, &graphics, &device, descriptor, extent)?;
        eprintln!(
            "shared_texture_timing adapter=\"{}\" extent={width}x{height} \
             vulkan_draw_us={:.1} composition_copy_gpu_us={:.1} fence_handoff_us={:.1}",
            descriptor.name,
            timing.vulkan_draw_microseconds,
            timing.copy_gpu_microseconds,
            timing.handoff_microseconds
        );
    }
    drop(graphics);
    drop(device);
    Ok(())
}

#[test]
fn every_vulkan_adapter_draws_shared_textures_that_composition_copies_exactly() -> TestResult {
    let _apartment = Apartment::enter()?;
    let (memory, mut execution) =
        harness::run_blocking(BUDGET, |context| Ok::<_, io::Error>(context.memory().clone()))
            .map_err(io)?;
    let descriptors = execution
        .asynchronous(2 * 1024 * 1024, |context| async move {
            let catalog = AdapterCatalog::discover(&context).await.map_err(io)?;
            Ok::<_, io::Error>(catalog.adapters().cloned().collect::<Vec<_>>())
        })
        .map_err(io)?;
    let adapters: Vec<_> = descriptors
        .into_iter()
        .filter(|adapter| {
            adapter.backend == Backend::Vulkan && adapter.class != AdapterClass::Software
        })
        .collect();
    if adapters.is_empty() {
        return Err("No hardware Vulkan adapter was enumerated".into());
    }
    let mut raw = std::ptr::null_mut();
    // SAFETY: The requested device IID and writable output slot belong to this test apartment.
    let status = unsafe {
        super::super::super::DCompositionCreateDevice2(
            std::ptr::null_mut(),
            &IDCompositionDevice::IID,
            &mut raw,
        )
    };
    let mut composition: Option<IDCompositionDevice> = None;
    // SAFETY: This adopts the exact transferred device reference, including partial failure.
    unsafe { adopt(&mut composition, raw, status, "test_shared_canvas_device")? };
    let composition = composition.ok_or("Composition device missing")?;
    let failures: Vec<String> = adapters
        .iter()
        .filter_map(|adapter| {
            qualify_adapter(&mut execution, &composition, &memory, adapter)
                .err()
                .map(|cause| format!("{}: {cause}", adapter.name))
        })
        .collect();
    drop(composition);
    drop(memory);
    execution.finish().map_err(io)?;
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ").into()) }
}
