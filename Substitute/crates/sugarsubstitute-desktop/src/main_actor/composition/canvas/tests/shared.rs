// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify Vulkan drawing into shared textures and their composition copy on every GPU.

use super::{
    super::timing::{Reach, Source, UploadCost},
    *,
};
use crate::canvas::{CanvasGraphics, SharedCanvasTexture, SharedFrame};
use std::{
    error::Error,
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_canvas::{DirtyRegions, Viewport};
use sugarsubstitute_image_ops::Region;
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
const WARM_CPU_FRAMES: u64 = 24;

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
    ///
    /// The GPU reports each result or a device failure, which `?` preserves, so the spin has no
    /// wall-clock bound: a slow or contended GPU only makes the test slower.
    fn finish(self) -> TestResult<f64> {
        // SAFETY: These queries belong to this context's device.
        unsafe {
            self.context.End(&self.end);
            self.context.End(&self.disjoint);
            self.context.Flush();
        }
        let mut disjoint = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
        let (mut start, mut end) = (0_u64, 0_u64);
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
    cpu_fill_microseconds: f64,
    cpu_draw_microseconds: f64,
    cpu_bitmap_creations: usize,
    whole_bytes: u64,
    local: Repair,
    pan: Repair,
    shared_open: f64,
    shared_wrap: f64,
    shared_draw: f64,
    shared_commit: f64,
}

/// Warm medians of one kind of repaired frame, in microseconds, and the bytes each copied.
struct Repair {
    bytes: u64,
    fill: f64,
    draw: f64,
    commit: f64,
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
    let mut canvas =
        Canvas::new(composition.clone(), root, memory.clone(), graphics.clone(), true)?;
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
    // The interval above already waited for the GPU, so the hand-off is timed on a frame of its
    // own: presented, then awaited at once, as a worker reusing the texture would.
    let handed = stamp(3)?;
    assert_eq!(canvas.present(&shared(handed), placement)?, Some(handed));
    let handoff_texture = texture.clone();
    let handoff = execution
        .blocking(64 * 1024, move |context| {
            let started = Instant::now();
            handoff_texture.await_reads(&context).map_err(io)?;
            Ok::<_, io::Error>(started.elapsed())
        })
        .map_err(io)?;

    let capture = |canvas: &mut Canvas, content: &CanvasContent<'_>, expected: &[u32]| -> TestResult {
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
    capture(&mut canvas, &shared(stamp(4)?), &pixels)?;
    // CPU-prepared previews and GPU frames alternate on the same retained surface.
    let cpu_pixels = vec![0xff20_4060; pixels.len()];
    let damage = DamagePlan::full(extent);
    let cpu = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp(5)?,
        base: None,
        damage: &damage,
        pixels: &cpu_pixels,
    };
    capture(&mut canvas, &CanvasContent::Cpu(cpu), &cpu_pixels)?;
    capture(&mut canvas, &shared(stamp(6)?), &pixels)?;
    assert!(canvas.active.as_ref().is_some_and(|allocation| allocation.extent() == extent));
    assert!(!canvas.has_retirements());

    // Regressed generations and textures exported by another composition device are rejected
    // before any native effect; the committed frame remains the acknowledged content.
    assert!(canvas.present(&shared(stamp(5)?), placement).is_err());
    assert!(canvas.retains(stamp(6)?, placement));
    let (foreign_memory, selected) = (memory.clone(), descriptor.clone());
    let foreign = execution.blocking(64 * 1024, move |context| {
        let graphics =
            CanvasGraphics::prepare(&foreign_memory, &context, Some(&selected)).map_err(io)?;
        SharedCanvasTexture::create(&factory(&graphics)?, extent, &context).map_err(io)
    });
    let foreign = foreign.map_err(io)?;
    let foreign_frame =
        CanvasContent::Shared(SharedFrame { pending: true, target: stamp(7)?, texture: &foreign });
    assert!(canvas.present(&foreign_frame, placement).is_err());
    assert!(canvas.retains(stamp(6)?, placement));
    drop(foreign);

    // Warm preview replacement: distinct complete CPU frames on the one retained surface.
    let alternate = vec![0xff60_4020; pixels.len()];
    let mut samples = Vec::new();
    for round in 0..WARM_CPU_FRAMES {
        let frame = CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: true,
            target: stamp(10 + round)?,
            base: None,
            damage: &damage,
            pixels: if round % 2 == 0 { &cpu_pixels } else { &alternate },
        };
        assert_eq!(canvas.present(&CanvasContent::Cpu(frame), placement)?, Some(frame.target));
        samples.push(canvas.timings.latest.ok_or("Presentation cost absent")?);
    }
    let median = |cost: fn(&super::timing::UploadCost) -> std::time::Duration| {
        let mut values: Vec<_> = samples.iter().map(cost).collect();
        values.sort_unstable();
        values[values.len() / 2].as_secs_f64() * 1_000_000.0
    };
    let (cpu_fill_microseconds, cpu_draw_microseconds) =
        (median(|cost| cost.fill), median(|cost| cost.draw));
    let cpu_bitmap_creations =
        samples.iter().filter(|cost| cost.source == super::timing::Source::Created).count();
    // A shared frame sits between the earlier CPU frame and these; the surface's own bitmap
    // survives it, so no warm preview allocates.
    assert_eq!(cpu_bitmap_creations, 0);
    let mut generation = 9 + WARM_CPU_FRAMES;

    // Warm localized change: each frame is relative to the one before and changes one region
    // around the viewport's centre, so only that region is copied and drawn.
    let centre = Region::new(extent.width() / 2, extent.height() / 2, 1, 1)?;
    let mut touched = DirtyRegions::empty(extent);
    touched.insert(centre);
    let mut local = DamagePlan::unchanged(extent);
    local.include_document_damage(&touched, Viewport::new(extent)?)?;
    let local_bytes: u64 = local.dirty().regions().map(|region| region.pixels() * 4).sum();
    // Warm pan: the retained pixels move sixteen device pixels and one strip is exposed.
    let pan = DamagePlan::pan(extent, 16.min(extent.width() as i32 / 4), 0)?;
    let pan_bytes: u64 = pan.dirty().regions().map(|region| region.pixels() * 4).sum();
    let warm = |canvas: &mut Canvas,
                generation: &mut u64,
                plan: &DamagePlan,
                scrolled: bool|
     -> TestResult<(f64, f64, f64)> {
        let mut costs = Vec::new();
        for round in 0..WARM_CPU_FRAMES {
            *generation += 1;
            let frame = CpuFrame {
                kind: CpuFrameKind::Prepared,
                pending: true,
                target: stamp(*generation)?,
                base: Some(stamp(*generation - 1)?),
                damage: plan,
                pixels: if round % 2 == 0 { &alternate } else { &cpu_pixels },
            };
            assert_eq!(canvas.present(&CanvasContent::Cpu(frame), placement)?, Some(frame.target));
            let cost = canvas.timings.latest.ok_or("Presentation cost absent")?;
            assert_eq!(
                cost.reach,
                Reach::Repaired { scrolled, regions: plan.dirty().regions().count() as u32 }
            );
            costs.push((cost, canvas.timings.latest_commit.ok_or("Commit cost absent")?));
        }
        let middle = |value: fn(&(UploadCost, Duration)) -> Duration| {
            let mut values: Vec<_> = costs.iter().map(value).collect();
            values.sort_unstable();
            values[values.len() / 2].as_secs_f64() * 1_000_000.0
        };
        Ok((
            middle(|(cost, _)| cost.open + cost.fill),
            middle(|(cost, _)| cost.draw),
            middle(|(_, commit)| *commit),
        ))
    };
    let (local_fill, local_draw, local_commit) = warm(&mut canvas, &mut generation, &local, false)?;
    let (pan_fill, pan_draw, pan_commit) = warm(&mut canvas, &mut generation, &pan, true)?;

    // Warm shared frames: what opening the update, wrapping the texture, drawing it and
    // committing each cost on the UI thread.
    let mut wrapped = Vec::new();
    for _ in 0..WARM_CPU_FRAMES {
        generation += 1;
        let frame = stamp(generation)?;
        assert_eq!(canvas.present(&shared(frame), placement)?, Some(frame));
        let cost = canvas.timings.latest.ok_or("Presentation cost absent")?;
        assert_eq!((cost.source, cost.reach), (Source::Wrapped, Reach::Whole));
        wrapped.push((cost, canvas.timings.latest_commit.ok_or("Commit cost absent")?));
    }
    let shared_middle = |value: fn(&(UploadCost, Duration)) -> Duration| {
        let mut values: Vec<_> = wrapped.iter().map(value).collect();
        values.sort_unstable();
        values[values.len() / 2].as_secs_f64() * 1_000_000.0
    };

    // CPU A, shared B, CPU C relative to B: the surface holds B, so C repairs it; the CPU
    // bitmap still holds A, so only C's own region is trusted from it, and the next whole frame
    // is copied completely instead of being patched onto A.
    let surface_frame = stamp(generation)?;
    generation += 1;
    let relative = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp(generation)?,
        base: Some(surface_frame),
        damage: &local,
        pixels: &alternate,
    };
    canvas.capture_next = true;
    assert_eq!(canvas.present(&CanvasContent::Cpu(relative), placement)?, Some(relative.target));
    let repaired = canvas.timings.latest.ok_or("Presentation cost absent")?;
    assert_eq!(repaired.reach, Reach::Repaired { scrolled: false, regions: 1 });
    assert_eq!(repaired.source_bytes, local_bytes);
    let region = local.dirty().regions().next().ok_or("Local change reaches no pixel")?;
    let captured = canvas
        .active
        .as_mut()
        .and_then(|allocation| allocation.capture.take())
        .ok_or("Repair capture absent")?;
    assert_eq!(captured, vec![alternate[0]; region.pixels() as usize]);
    assert_eq!(
        canvas.active.as_ref().map(Allocation::holds),
        Some((Some(relative.target), None))
    );
    generation += 1;
    let complete = CpuFrame {
        kind: CpuFrameKind::Prepared,
        pending: true,
        target: stamp(generation)?,
        base: None,
        damage: &damage,
        pixels: &cpu_pixels,
    };
    capture(&mut canvas, &CanvasContent::Cpu(complete), &cpu_pixels)?;
    assert_eq!(
        canvas.timings.latest.map(|cost| (cost.reach, cost.source_bytes)),
        Some((Reach::Whole, extent.pixels() * 4))
    );
    let last = stamp(generation)?;

    // Hiding withdraws the shared frame; completion follows the compositor's retirement.
    assert!(!canvas.hide()?);
    assert!(!canvas.retains(last, placement));
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
        cpu_fill_microseconds,
        cpu_draw_microseconds,
        cpu_bitmap_creations,
        whole_bytes: extent.pixels() * 4,
        local: Repair { bytes: local_bytes, fill: local_fill, draw: local_draw, commit: local_commit },
        pan: Repair { bytes: pan_bytes, fill: pan_fill, draw: pan_draw, commit: pan_commit },
        shared_open: shared_middle(|(cost, _)| cost.open),
        shared_wrap: shared_middle(|(cost, _)| cost.fill),
        shared_draw: shared_middle(|(cost, _)| cost.draw),
        shared_commit: shared_middle(|(_, commit)| *commit),
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
             vulkan_draw_us={:.1} composition_copy_gpu_us={:.1} fence_handoff_us={:.1} \
             warm_cpu_fill_us={:.1} warm_cpu_draw_us={:.1} warm_cpu_bitmap_creations={}/{}",
            descriptor.name,
            timing.vulkan_draw_microseconds,
            timing.copy_gpu_microseconds,
            timing.handoff_microseconds,
            timing.cpu_fill_microseconds,
            timing.cpu_draw_microseconds,
            timing.cpu_bitmap_creations,
            WARM_CPU_FRAMES
        );
        // Medians of the warm frames, in microseconds on the UI thread, and bytes copied per frame.
        eprintln!(
            "presenter_timing adapter=\"{}\" extent={width}x{height} whole_bytes={} \
             local_bytes={} local_fill_us={:.1} local_draw_us={:.1} local_commit_us={:.1} \
             pan_bytes={} pan_fill_us={:.1} pan_draw_us={:.1} pan_commit_us={:.1} \
             shared_open_us={:.1} shared_wrap_us={:.1} shared_draw_us={:.1} shared_commit_us={:.1}",
            descriptor.name,
            timing.whole_bytes,
            timing.local.bytes,
            timing.local.fill,
            timing.local.draw,
            timing.local.commit,
            timing.pan.bytes,
            timing.pan.fill,
            timing.pan.draw,
            timing.pan.commit,
            timing.shared_open,
            timing.shared_wrap,
            timing.shared_draw,
            timing.shared_commit
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
