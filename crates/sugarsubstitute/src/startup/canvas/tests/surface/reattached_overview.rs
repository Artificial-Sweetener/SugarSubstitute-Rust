// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer an overview on a replacement attachment in every worker completion order.

use super::*;
use std::{collections::VecDeque, sync::atomic::AtomicBool};
use sugarsubstitute_contracts::execution::{
    CpuStep, CpuTask, JobContext, ReplacementKey, Scope, SurfaceId,
};
use viewport::fixture::{Fixture, image as output_image};

mod acquisition;
mod comparison_exit;

const CANVAS: u64 = 1;
const GATES: u64 = 2;
/// Display replacement lane owned by the gate, distinct from every canvas display surface.
const GATE_SURFACE: u64 = 9;

/// Execution lane that one canvas stage waits behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    /// Source acquisition runs on the ordinary infrastructure lane.
    Load,
    /// Canonical backing preparation runs on the decode CPU lane.
    Render,
    /// Display projection runs on the interactive CPU lane.
    Display,
}

/// Opened once by the test; read by the job holding its lane or its acquisition.
#[derive(Default)]
struct Latch {
    open: Mutex<bool>,
    changed: Condvar,
    entered: AtomicBool,
    /// Wake the polling host once a running canvas job reaches this latch.
    announce: Option<Arc<Wake>>,
}

impl Latch {
    /// Hold work inside a running canvas job, announcing its arrival to the polling host.
    fn announcing(wake: Arc<Wake>) -> Self {
        Self { announce: Some(wake), ..Self::default() }
    }

    fn open(&self) {
        *self.open.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.changed.notify_all();
    }

    /// Hold a blocking worker until opened. `Gates` opens every latch when dropped, so a failed
    /// test cannot strand shutdown.
    fn wait(&self) -> Result<(), std::io::Error> {
        self.entered.store(true, Ordering::SeqCst);
        if let Some(wake) = &self.announce {
            wake.wake().map_err(std::io::Error::other)?;
        }
        let guard = self.open.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(crate::completion_wake::wait_while_watched(
            &self.changed,
            guard,
            || "the test to open a held lane".to_owned(),
            |open| !*open,
        ));
        Ok(())
    }
}

/// Occupy a cooperative CPU lane until the latch opens, blocking rather than spinning a lane
/// that parallel tests share.
struct HeldCpu(Arc<Latch>);

impl CpuTask<(), std::io::Error> for HeldCpu {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<()>, std::io::Error> {
        self.0.wait().map(|()| CpuStep::Complete(()))
    }
}

/// One held lane and the job occupying it, or a latch inside the canvas stage itself.
struct Gate {
    stage: Stage,
    latch: Arc<Latch>,
    job: Option<OwnedJob<(), std::io::Error>>,
}

/// Lanes held by another scope of the canvas's execution owner, opened in a chosen order.
struct Gates {
    pending: VecDeque<Gate>,
    opened: Vec<Gate>,
    _scope: Scope,
}

impl Gates {
    /// Occupy every lane before the canvas admits its stage, so each stage queues behind it.
    /// An `acquisition` latch instead holds the load stage inside its running acquisition.
    fn hold(
        fixture: &Fixture,
        order: [Stage; 3],
        acquisition: Option<&Arc<Latch>>,
    ) -> Result<Self, Box<dyn Error>> {
        let scope = fixture
            .client
            .create_scope(ScopeSpec { id: ScopeId::new(GATES)?, deadline: None }, None)?;
        let mut pending = VecDeque::new();
        for stage in order {
            if stage == Stage::Load
                && let Some(latch) = acquisition
            {
                pending.push_back(Gate { stage, latch: latch.clone(), job: None });
                continue;
            }
            let latch = Arc::new(Latch::default());
            let (operation, class) = match stage {
                Stage::Load => (Operation::Infrastructure, JobClass::InfrastructureOrdinary),
                Stage::Render => (Operation::ImageEvaluate, JobClass::CpuDecode),
                Stage::Display => (Operation::FramePrepare, JobClass::CpuInteractive),
            };
            let spec = JobSpec {
                id: fixture.canvas.jobs.allocate()?,
                operation,
                class,
                deadline: Some(fixture.clock.now().checked_add(Duration::from_secs(60))?),
                reservation_bytes: 4096,
                // Interactive admission requires a replacement lane; this one is the gate's own.
                replacement: (class == JobClass::CpuInteractive).then_some(ReplacementKey {
                    surface: SurfaceId::new(GATE_SURFACE)?,
                    revision: Generation::new(1)?,
                }),
            };
            let held = latch.clone();
            let job = match stage {
                Stage::Load => {
                    fixture.client.submit_blocking(&scope, spec, move |_| held.wait())?
                }
                Stage::Render | Stage::Display => {
                    fixture.client.submit_cpu(&scope, spec, move |_| HeldCpu(held))?
                }
            };
            pending.push_back(Gate { stage, latch, job: Some(job) });
        }
        Ok(Self { pending, opened: Vec::new(), _scope: scope })
    }

    fn holding(&self) -> bool {
        !self.pending.is_empty()
    }

    fn open_next(&mut self) -> bool {
        let Some(gate) = self.pending.pop_front() else { return false };
        gate.latch.open();
        self.opened.push(gate);
        true
    }

    /// Canvas jobs running only because a held latch inside them has not opened.
    fn blocking(&self) -> usize {
        self.pending
            .iter()
            .filter(|gate| gate.job.is_none() && gate.latch.entered.load(Ordering::SeqCst))
            .count()
    }

    /// Require every gate job's own successful outcome before releasing its scope.
    fn finish(mut self, fixture: &Fixture) -> Result<(), Box<dyn Error>> {
        while self.open_next() {}
        for gate in &mut self.opened {
            let Some(job) = &mut gate.job else { continue };
            let outcome = loop {
                let observed = fixture.wake.revision();
                fixture.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
                if let Some(completion) = job.try_take_completion() {
                    break completion.outcome;
                }
                fixture.wake.wait_after(observed);
            };
            assert!(matches!(outcome, JobOutcome::Success(())), "{:?} gate failed", gate.stage);
        }
        Ok(())
    }
}

impl Drop for Gates {
    fn drop(&mut self) {
        for gate in self.pending.iter().chain(&self.opened) {
            gate.latch.open();
        }
    }
}

/// Host conditions around the replacement attachment.
#[derive(Clone, Copy, Debug)]
struct Variant {
    /// Run 463's density, geometry and allocation with two identical 1024x1024 outputs.
    large: bool,
    /// Zoom the first attachment so refinement work precedes its release.
    navigate: bool,
    /// Hold lanes from before release, across retirement of the first attachment's work.
    gate_release: bool,
    /// Execution-clock milliseconds added before every poll.
    clock_step: u64,
    /// Acquire the overview through a host adapter retaining encoded originals in cache custody.
    acquired: bool,
}

/// Poll only after a wake or at a reported deadline. A held lane opens only when the canvas has
/// no other progress, so each stage is observed waiting behind its lane; work that has neither
/// a wake nor a deadline fails instead of waiting for an unrelated host event.
fn await_reply(
    fixture: &mut Fixture,
    gates: &mut Gates,
    id: u64,
    status: &str,
    clock_step: u64,
) -> Result<(), Box<dyn Error>> {
    let canvas = ScopeId::new(CANVAS)?;
    loop {
        let observed = fixture.wake.revision();
        fixture.advance(clock_step);
        fixture.pump()?;
        if let Some(command) = fixture.canvas.take_command() {
            return check_reply(command, id, status);
        }
        let work = fixture.scope_work(canvas)?;
        if work.running > gates.blocking() || (work.queued > 0 && !gates.holding()) {
            fixture.wake.wait_after(observed);
        } else if gates.open_next() {
        } else if let Some(deadline) = fixture.canvas.next_deadline() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            fixture.advance(u64::try_from(remaining.as_millis())?.saturating_add(1));
        } else if work.undelivered > 0 {
            fixture.wake.wait_after(observed);
        } else {
            return Err(format!("Request {id} stalled without work, wake or deadline").into());
        }
    }
}

fn geometry(width: f64, height: f64, density: f64, visible: bool) -> Value {
    let clip = if visible { (width, height) } else { (0.0, 0.0) };
    json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":902.6875,"y":78},
        "viewport":{"width":width,"height":height},
        "clip":{"x":902.6875,"y":78,"width":clip.0,"height":clip.1},
        "physicalPixelsPerCssPixel":density, "visible":visible, "focused":false
    }})
}

/// Commit and release a selection, then show identical overview cells on the next attachment.
fn reattached_overview(order: [Stage; 3], variant: Variant) -> Result<(), Box<dyn Error>> {
    let density = if variant.large { 1.75 } else { 1.0 };
    let mut fixture = Fixture::with_density(density)?;
    let (side, cells, candidate) = if variant.large {
        (1024, [[191.83, 0.0, 214.79, 214.79], [191.83, 644.36, 214.79, 214.79]], (598.45, 859.14))
    } else {
        (320, [[0.0, 0.0, 40.0, 40.0], [40.0, 0.0, 40.0, 40.0]], (80.0, 40.0))
    };
    output_image(&fixture.output("first.png"), side, side, [200, 100, 50])?;
    output_image(&fixture.output("second.png"), side, side, [200, 100, 50])?;
    fixture.commit(4)?;
    let mut id = 5;
    if variant.navigate {
        fixture.viewport(id, json!({"type":"zoom","factor":2}))?;
        fixture.complete(id)?;
        id += 1;
    }
    if variant.large {
        fixture.rebound(PhysicalRect { x: 0, y: 0, width: 2640, height: 1680 });
        fixture.send(id, "surface.geometry", geometry(candidate.0, candidate.1, density, true))?;
        fixture.complete(id)?;
        id += 1;
    }
    let held_acquisition = Arc::new(Latch::announcing(fixture.wake.clone()));
    if variant.acquired {
        fixture.canvas.surface.acquire_overviews_with(acquisition::held(held_acquisition.clone()));
    }
    let acquisition = variant.acquired.then_some(&held_acquisition);
    let mut gates =
        if variant.gate_release { Some(Gates::hold(&fixture, order, acquisition)?) } else { None };
    fixture.send(id, "surface.release", json!({}))?;
    fixture.take(id, "applied")?;
    id += 1;
    fixture.send(id, "surface.prepare", json!({}))?;
    match &mut gates {
        Some(gates) => await_reply(&mut fixture, gates, id, "ready", variant.clock_step)?,
        None => fixture.wait_reply(id, "ready")?,
    }
    id += 1;
    fixture.send(id, "surface.geometry", geometry(candidate.0, candidate.1, density, false))?;
    fixture.take(id, "applied")?;
    id += 1;
    let mut gates = match gates {
        Some(gates) => gates,
        None => Gates::hold(&fixture, order, acquisition)?,
    };
    let cell = |filename: &str, [x, y, width, height]: [f64; 4]| {
        json!({"itemId":format!("result:{filename}"),
            "file":{"filename":filename,"subfolder":"","type":"output"},
            "rect":{"x":x,"y":y,"width":width,"height":height}})
    };
    fixture.send(
        id,
        "surface.overview",
        json!({"cells":[cell("first.png", cells[0]), cell("second.png", cells[1])]}),
    )?;
    await_reply(&mut fixture, &mut gates, id, "applied", variant.clock_step)
        .map_err(|cause| format!("{order:?} {variant:?}: {cause}"))?;
    assert_eq!(held_acquisition.entered.load(Ordering::SeqCst), variant.acquired);
    gates.finish(&fixture)?;
    fixture.finish()
}

/// Every order in which the load, render and display lanes can become available.
const ORDERS: [[Stage; 3]; 6] = {
    use Stage::{Display, Load, Render};
    [
        [Load, Render, Display],
        [Load, Display, Render],
        [Render, Load, Display],
        [Render, Display, Load],
        [Display, Load, Render],
        [Display, Render, Load],
    ]
};

#[test]
fn a_reattached_overview_is_applied_in_every_worker_completion_order() -> Result<(), Box<dyn Error>>
{
    for variant in [
        Variant {
            large: false,
            navigate: false,
            gate_release: false,
            clock_step: 0,
            acquired: false,
        },
        Variant { large: true, navigate: true, gate_release: true, clock_step: 0, acquired: false },
        Variant {
            large: true,
            navigate: false,
            gate_release: false,
            clock_step: 5,
            acquired: false,
        },
        Variant {
            large: false,
            navigate: false,
            gate_release: false,
            clock_step: 0,
            acquired: true,
        },
        Variant { large: true, navigate: true, gate_release: false, clock_step: 0, acquired: true },
    ] {
        for order in ORDERS {
            reattached_overview(order, variant)?;
        }
    }
    Ok(())
}
