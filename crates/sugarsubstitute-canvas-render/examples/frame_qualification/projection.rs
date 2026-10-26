// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure bounded 4K software projection through actual interactive execution admission.
use super::*;
use sugarsubstitute_canvas::{DamagePlan, FrameStamp, ViewProjection, Viewport};
use sugarsubstitute_canvas_render::{CpuSurface, PreparedUpdate, ProjectionBuffer};
use sugarsubstitute_image_ops::{Extent, Pixel};

const SAMPLES: usize = 32;
const CASES: usize = 8;
type Failure = Box<dyn Error + Send + Sync>;

/// One finite case history; no per-frame dumps or external storage are required.
struct Report {
    elapsed: [[Duration; SAMPLES]; CASES],
    checksum: u64,
    _memory: MemoryClaim,
}
/// Prepare one canonical viewport, then yield after each disposable display projection.
struct Task {
    base: Option<CpuSurface>,
    report: Option<Report>,
    view: Viewport,
    phase: usize,
    scratch: Option<ProjectionBuffer>,
}
impl CpuTask<Report, Failure> for Task {
    fn step(&mut self, context: &JobContext) -> std::result::Result<CpuStep<Report>, Failure> {
        let stamp = |generation| -> std::result::Result<FrameStamp, Failure> {
            Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? })
        };
        if self.base.is_none() {
            self.report = Some(Report {
                elapsed: [[Duration::ZERO; SAMPLES]; CASES],
                checksum: 0,
                _memory: context.memory().claim(64 * 1024)?,
            });
            self.base = Some(CpuSurface::from_prepared(PreparedUpdate::prepare(
                stamp(1)?,
                None,
                DamagePlan::full(self.view.surface()),
                context.memory(),
                |region, pixels| {
                    for y in 0..region.height() {
                        for x in 0..region.width() {
                            pixels[(y * region.width() + x) as usize] =
                                0xff000000 | ((x % 251) << 16) | ((y % 239) << 8) | ((x + y) % 233);
                        }
                    }
                    Ok(())
                },
            )?)?);
            return Ok(CpuStep::Yield);
        }
        if self.phase == CASES * (SAMPLES + 2) {
            drop(self.scratch.take());
            drop(self.base.take());
            return Ok(CpuStep::Complete(self.report.take().ok_or("Projection report absent")?));
        }
        let case = self.phase / (2 * (SAMPLES + 2)) * 2 + self.phase % 2;
        let sample = self.phase / 2 % (SAMPLES + 2);
        let reuse = case % 2 == 1;
        let mut target = self.view;
        let center = [1920.25, 1080.125];
        match case / 2 {
            0 => target.pan_by([13.0 + sample as f64, -7.0])?,
            1 => target.zoom_at(1.05 + sample as f64 / 1000.0, center)?,
            2 => target.zoom_at(0.8 - sample as f64 / 1000.0, center)?,
            _ => target.zoom_at(8.0 + sample as f64 / 10.0, center)?,
        }
        let started = Instant::now();
        let base = self.base.as_ref().ok_or("Projection canonical frame absent")?;
        let mapping = ViewProjection::new(self.view, target)?;
        let storage = match if reuse { self.scratch.take() } else { None } {
            Some(storage) => storage,
            None => ProjectionBuffer::new(mapping, context.memory(), context)?,
        };
        let frame = storage.project(
            stamp(self.phase as u64 + 2)?,
            base,
            mapping,
            Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
            context,
        )?;
        let pixels = std::hint::black_box(frame.pixels());
        let value = u64::from(pixels[0]) ^ u64::from(pixels[pixels.len() / 2]);
        if reuse {
            self.scratch = Some(frame.into_buffer());
        } else {
            drop(frame);
        }
        let elapsed = started.elapsed();
        let report = self.report.as_mut().ok_or("Projection report absent")?;
        report.checksum = report.checksum.wrapping_add(value);
        if sample >= 2 {
            report.elapsed[case][sample - 2] = elapsed;
        }
        self.phase += 1;
        Ok(CpuStep::Yield)
    }
}

/// Qualify CPU projection without discovering a GPU or constructing a native window.
pub(super) fn qualify(
    client: &sugarsubstitute_execution::ExecutionClient,
    scope: &Scope,
    wake: &Wake,
    deadline: Moment,
) -> Result {
    let view = Viewport::new(Extent::new(3840, 2160)?)?;
    let mut job = client.submit_cpu(
        scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::FramePrepare,
            class: JobClass::CpuInteractive,
            deadline: Some(deadline),
            reservation_bytes: 96 * 1024 * 1024,
            replacement: Some(ReplacementKey {
                surface: SurfaceId::new(1)?,
                revision: Generation::new(1)?,
            }),
        },
        move |_| Task { base: None, report: None, view, phase: 0, scratch: None },
    )?;
    let completion = wake.until(None, || {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(job.try_take_completion())
    })?;
    let report = match completion.outcome {
        JobOutcome::Success(report) => report,
        JobOutcome::Failure(cause) => return Err(cause),
        other => return Err(format!("CPU projection stopped: {:?}", other.kind()).into()),
    };
    for index in 0..CASES {
        let case = ["integer-pan", "zoom-in", "zoom-out", "magnify"][index / 2];
        let allocation = if index % 2 == 1 { "reused" } else { "fresh" };
        let mut times = report.elapsed[index];
        times.sort_unstable();
        println!(
            "CPU_PROJECTION: case={case} allocation={allocation} extent=3840x2160 samples={SAMPLES} min_us={} p50_us={} p95_us={} max_us={} checksum={}",
            times[0].as_micros(),
            times[SAMPLES / 2].as_micros(),
            times[(SAMPLES * 95 / 100).min(SAMPLES - 1)].as_micros(),
            times[SAMPLES - 1].as_micros(),
            report.checksum
        );
    }
    Ok(())
}
