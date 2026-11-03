// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify one explicitly selected Vulkan device through the production offscreen blitter.
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Operation as ExecutionOperation, *};
use sugarsubstitute_execution::{ExecutionOwner, MonotonicClock};
use sugarsubstitute_image_gpu::{
    AdapterCatalog, AdapterDescriptor, AdapterKey, DisplayImage, DisplayMapping, GpuDevice,
    GpuEvaluator, OverlayPixel,
    boundary_fixtures::{
        CompositeFixture, CompositeQualification, CompositeSource, qualify_composite,
    },
};
use sugarsubstitute_image_ops::*;

type Result<T = ()> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const PRIMARY: AssetId = AssetId([71; 32]);
const FALLBACK: AssetId = AssetId([72; 32]);

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut revision = self.0.lock().map_err(|_| {
            ExecutionError::new(
                ExecutionOperation::Infrastructure,
                Box::new(io::Error::other("Composite qualification wake poisoned")),
            )
        })?;
        *revision += 1;
        self.1.notify_all();
        Ok(())
    }
}
impl Wake {
    fn until<T>(
        &self,
        poll: Option<Duration>,
        mut observe: impl FnMut() -> Result<Option<T>>,
    ) -> Result<T> {
        let cutoff = Instant::now() + Duration::from_secs(90);
        loop {
            let revision = *self.0.lock().map_err(|_| "Composite qualification wake poisoned")?;
            if let Some(value) = observe()? {
                return Ok(value);
            }
            let remaining = cutoff
                .checked_duration_since(Instant::now())
                .ok_or("Composite qualification observation expired")?;
            let guard = self.0.lock().map_err(|_| "Composite qualification wake poisoned")?;
            let (guard, timeout) = self
                .1
                .wait_timeout_while(
                    guard,
                    poll.map_or(remaining, |poll| poll.min(remaining)),
                    |current| *current == revision,
                )
                .map_err(|_| "Composite qualification wake poisoned")?;
            drop(guard);
            if timeout.timed_out() && Instant::now() >= cutoff {
                return observe()?
                    .ok_or_else(|| "Composite qualification observation expired".into());
            }
        }
    }
}

struct Selected {
    device: Arc<GpuDevice>,
}
struct Report {
    adapter: AdapterDescriptor,
    qualification: CompositeQualification,
    video_memory: Option<sugarsubstitute_image_gpu::VideoMemoryBudget>,
}

fn main() -> Result {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let key = match arguments.as_slice() {
        [flag, key] if flag == "--adapter" => key.parse::<AdapterKey>()?,
        _ => return Err("Use composite_qualification --adapter <vulkan:vendor:device>".into()),
    };
    if !key.to_string().starts_with("vulkan:") {
        return Err("Composite qualification requires an explicit Vulkan adapter key".into());
    }
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let deadline = client.now()?.checked_add(Duration::from_secs(60))?;
    let result = (|| -> Result {
        let mut selection = client.submit_async(
            &scope,
            JobSpec {
                id: JobId::new(1)?,
                operation: ExecutionOperation::Infrastructure,
                class: JobClass::IoOrdinary,
                deadline: Some(deadline),
                reservation_bytes: 64 * 1024 * 1024,
                replacement: None,
            },
            move |context| async move {
                let catalog = AdapterCatalog::discover(&context).await?;
                let id = catalog.resolve(key)?;
                let device = catalog.select(id, context.memory(), &context).await?;
                Ok::<Selected, Box<dyn Error + Send + Sync>>(Selected { device })
            },
        )?;
        let JobCompletion { outcome, .. } = wake.until(None, || {
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            Ok(selection.try_take_completion())
        })?;
        let selected = match outcome {
            JobOutcome::Success(selected) => selected,
            JobOutcome::Failure(cause) => return Err(cause),
            other => return Err(format!("GPU selection outcome: {:?}", other.kind()).into()),
        };
        drop(selection);
        let mut input = Some(selected);
        let mut qualification = client.submit_blocking_owned_input(
            &scope,
            JobSpec {
                id: JobId::new(2)?,
                operation: ExecutionOperation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(deadline),
                reservation_bytes: 32 * 1024 * 1024,
                replacement: None,
            },
            &mut input,
            |selected, context| run(selected.device, &context),
        )?;
        let JobCompletion { outcome, .. } = wake.until(None, || {
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            Ok(qualification.try_take_completion())
        })?;
        drop(qualification);
        match outcome {
            JobOutcome::Success(report) => {
                let (budget, usage) = report
                    .video_memory
                    .map_or((0, 0), |memory| (memory.budget_bytes, memory.usage_bytes));
                println!(
                    "SUCCESS: adapter={} key={} pixels={} pixel_hash=fnv1a64:{:016x} video_budget_bytes={budget} video_usage_bytes={usage}",
                    report.adapter.name,
                    report.adapter.key(),
                    report.qualification.pixels,
                    report.qualification.pixel_hash,
                );
                Ok(())
            }
            JobOutcome::Failure(cause) => Err(cause),
            other => Err(format!("Composite qualification outcome: {:?}", other.kind()).into()),
        }
    })();
    scope.cancel();
    owner.request_shutdown(client.now()?.checked_add(Duration::from_secs(5))?);
    // Worker-thread termination has no completion notification; poll its observed shutdown state.
    let retirement = wake.until(Some(Duration::from_millis(16)), || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(owner.poll_shutdown()?.settled.then_some(()))
    });
    let retirement = retirement.and_then(|()| {
        owner.finish_shutdown()?;
        if scope.snapshot().reserved_bytes != 0 {
            return Err("Composite qualification retained execution admission".into());
        }
        Ok(())
    });
    match (result, retirement) {
        (Err(primary), Err(retirement)) => {
            Err(format!("{primary}; qualification retirement: {retirement}").into())
        }
        (Err(cause), _) | (_, Err(cause)) => Err(cause),
        _ => Ok(()),
    }
}

fn run(device: Arc<GpuDevice>, context: &JobContext) -> Result<Report> {
    let encoder = SrgbEncoder::new(context.memory())?;
    let primary =
        build_image(device.clone(), Extent::new(257, 3)?, PRIMARY, 17, &encoder, context)?;
    let fallback =
        build_image(device.clone(), Extent::new(4, 3)?, FALLBACK, 93, &encoder, context)?;
    let output = Extent::new(5, 4)?;
    let matte = [17, 29, 41, 255];
    let primary_columns = [
        DisplaySample::from_corner(201.75, 257, false),
        DisplaySample::from_corner(1.25, 257, true),
        DisplaySample::UNCOVERED,
        DisplaySample::from_corner(255.75, 257, true),
        DisplaySample::UNCOVERED,
    ];
    let primary_rows = [
        DisplaySample::from_corner(0.75, 3, false),
        DisplaySample::from_corner(1.25, 3, true),
        DisplaySample::UNCOVERED,
        DisplaySample::from_corner(2.75, 3, false),
    ];
    let fallback_columns = [
        DisplaySample::from_corner(0.5, 4, true),
        DisplaySample::from_corner(1.25, 4, true),
        DisplaySample::from_corner(2.5, 4, true),
        DisplaySample::from_corner(3.75, 4, true),
        DisplaySample::UNCOVERED,
    ];
    let fallback_rows = [
        DisplaySample::from_corner(0.5, 3, true),
        DisplaySample::from_corner(1.25, 3, true),
        DisplaySample::from_corner(2.25, 3, true),
        DisplaySample::UNCOVERED,
    ];
    let primary_mapping = DisplayMapping::new(
        primary.image.extent(),
        output,
        &primary_columns,
        &primary_rows,
        matte,
    )?;
    let fallback_mapping = DisplayMapping::new(
        fallback.image.extent(),
        output,
        &fallback_columns,
        &fallback_rows,
        matte,
    )?;
    let primary_source = CompositeSource::new(
        primary.image,
        primary_mapping,
        &primary.pixels,
        &primary_columns,
        &primary_rows,
    )?;
    let fallback_source = CompositeSource::new(
        fallback.image,
        fallback_mapping,
        &fallback.pixels,
        &fallback_columns,
        &fallback_rows,
    )?;
    // Selection ants and outline dashes spanning more than one overlay batch, in order: the
    // final explicit pixels cover everything drawn before them.
    let mut overlay = (0..600_000_u32)
        .map(|index| OverlayPixel {
            x: index % 5,
            y: (index / 5) % 4,
            color: [(index % 251) as u8, 77, (index % 13) as u8],
        })
        .collect::<Vec<_>>();
    overlay.extend([
        OverlayPixel { x: 0, y: 0, color: [245, 245, 245] },
        OverlayPixel { x: 2, y: 2, color: [25, 25, 25] },
        OverlayPixel { x: 4, y: 3, color: [255, 255, 255] },
        OverlayPixel { x: 2, y: 2, color: [0, 0, 0] },
    ]);
    let fixture =
        CompositeFixture::new(primary_source, fallback_source, matte)?.with_overlay(&overlay);
    let qualification = qualify_composite(fixture, context)?;
    let video_memory = device.video_memory_budget();
    Ok(Report { adapter: device.descriptor().clone(), qualification, video_memory })
}

struct ReferenceImage {
    image: DisplayImage,
    pixels: Vec<u32>,
    _memory: MemoryClaim,
}

fn build_image(
    device: Arc<GpuDevice>,
    extent: Extent,
    asset: AssetId,
    seed: u32,
    encoder: &SrgbEncoder,
    context: &JobContext,
) -> Result<ReferenceImage> {
    let mut graph = Graph::new(1, context.memory())?;
    let output = graph.add(sugarsubstitute_image_ops::Operation::Raster { asset, extent })?;
    let mut evaluator = GpuEvaluator::new(
        graph,
        Pattern { asset, seed },
        8,
        device.clone(),
        context.memory(),
        context,
    )?;
    let _regions_memory = context.memory().claim(extent.region().tile_count() * 32 + 128)?;
    let regions = extent.region().tiles().collect::<Vec<_>>();
    let displayed = evaluator.display_regions(output, &regions, context)?;
    let image = DisplayImage::compose(device, extent, None, None, displayed.tiles(), context)?;
    let memory = context.memory().claim(extent.pixels() * 4 + 128)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(extent.pixels() as usize)?;
    for y in 0..extent.height() {
        for x in 0..extent.width() {
            let [red, green, blue, _] = encoder.encode(pattern_pixel(seed, x, y)?)?;
            pixels.push(u32::from_be_bytes([255, red, green, blue]));
        }
    }
    Ok(ReferenceImage { image, pixels, _memory: memory })
}

struct Pattern {
    asset: AssetId,
    seed: u32,
}
impl RasterSource for Pattern {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        if asset != self.asset || output.len() as u64 != region.pixels() {
            return Err(ImageError::Invalid("Invalid composite qualification source request"));
        }
        for y in region.y()..region.bottom() {
            image_check(context)?;
            for x in region.x()..region.right() {
                output[((y - region.y()) * region.width() + x - region.x()) as usize] =
                    pattern_pixel(self.seed, x, y)?;
            }
        }
        Ok(())
    }
}

fn pattern_pixel(seed: u32, x: u32, y: u32) -> std::result::Result<Pixel, ImageError> {
    Pixel::linear(
        ((x * 37 + y * 11 + seed) % 256) as f32 / 255.0,
        ((x * 7 + y * 53 + seed * 3) % 256) as f32 / 255.0,
        ((x * 19 + y * 29 + seed * 5) % 256) as f32 / 255.0,
        1.0,
    )
}

fn image_check(context: &JobContext) -> std::result::Result<(), ImageError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled);
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline);
    }
    Ok(())
}
