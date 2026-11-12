// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A finished load held for parking is adopted once the outgoing document's work retires.

use super::*;
use crate::native_canvas::session::display::Displaying;
use sugarsubstitute_contracts::execution::JobPhase;

/// Keep one display step of the outgoing document on its CPU lane until the test opens it.
#[derive(Default)]
struct DisplayGate {
    open: Mutex<bool>,
    changed: Condvar,
}

impl DisplayGate {
    fn open(&self) {
        *self.open.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.changed.notify_all();
    }
}

/// Open the gate before the session joins its worker, including on assertion unwind.
struct OpenOnDrop(Arc<DisplayGate>);

impl Drop for OpenOnDrop {
    fn drop(&mut self) {
        self.0.open();
    }
}

struct HeldDisplay(Arc<DisplayGate>);

impl CpuTask<work::Draw, WorkError> for HeldDisplay {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<work::Draw>, WorkError> {
        let open = self
            .0
            .open
            .lock()
            .map_err(|cause| WorkError::new(std::io::Error::other(cause.to_string())))?;
        drop(crate::completion_wake::wait_while_watched(
            &self.0.changed,
            open,
            || "the test to open the held display".to_owned(),
            |open| !*open,
        ));
        // The fence's cancellation is observed on the next step.
        Ok(CpuStep::Yield)
    }
}

impl Fixture {
    /// Admit one display of the shown document whose step is held by `gate`.
    fn hold_display(&mut self, gate: &Arc<DisplayGate>) -> Result<(), Box<dyn Error>> {
        // Admission follows the display cadence, as the session's own submission does.
        if let PresentationReadiness::WaitingUntil(slot) =
            self.session.pacing.readiness(self.session.navigation_time())?
        {
            self.clock.0.store(u64::try_from(slot.as_nanos())?, Ordering::SeqCst);
        }
        let session = &mut self.session;
        if session.pacing.readiness(session.navigation_time())? != PresentationReadiness::Ready {
            return Err("The display cadence is busy".into());
        }
        let evidence = session.document.display_evidence.ok_or("No frame was shown")?;
        let generation = session.frame_revision.checked_add(1).ok_or("Frame revision exhausted")?;
        let target = FrameStamp { generation: Generation::new(generation)?, ..evidence.target };
        let spec = session.display_spec(target, false)?;
        let runtime = session.runtime.as_ref().ok_or("Canvas execution absent")?;
        let task = HeldDisplay(gate.clone());
        let job = runtime.client.submit_cpu(&runtime.scope, spec, move |_| task)?;
        let now = session.navigation_time();
        session.pacing.submitted(target, PresentationMode::Projected, now, now)?;
        session.frame_revision = generation;
        session.displaying = Some(Displaying {
            target,
            document: evidence.document,
            source_revision: evidence.source_revision,
            backdrop_generation: evidence.backdrop_generation,
            job,
        });
        Ok(())
    }

    /// Deliver completions and poll until `condition` holds, waiting only on real wakes.
    fn deliver_until(
        &mut self,
        condition: impl Fn(&Session) -> bool,
    ) -> Result<(), Box<dyn Error>> {
        for _ in 0..1000 {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.failed.is_some() {
                return Err("Canvas failed while holding a parked load".into());
            }
            if condition(&self.session) {
                return Ok(());
            }
            self.wake.wait_after(observed);
        }
        Err("The awaited canvas state never arrived".into())
    }

    /// Deliver completions without polling until the held display reaches its terminal.
    fn await_display_terminal(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            let displaying = self.session.displaying.as_ref().ok_or("Held display was lost")?;
            if matches!(displaying.job.status().phase, JobPhase::Terminal(_)) {
                return Ok(());
            }
            self.wake.wait_after(observed);
        }
    }
}

#[test]
fn a_load_held_for_parking_is_adopted_in_the_pass_that_retires_the_outgoing_display()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    let gate = Arc::new(DisplayGate::default());
    let opened = OpenOnDrop(gate.clone());
    fixture.hold_display(&gate)?;

    let mut source = Some(fixture.output([0, 255, 0])?);
    assert!(fixture.session.offer_source(&mut source, CacheReuse::Compatible, None)?);
    fixture.session.retain_selection(key("b"));
    // The finished load waits while the fence cancels the outgoing document's display.
    fixture.deliver_until(|session| {
        session
            .loading
            .as_ref()
            .is_some_and(|loading| matches!(loading.job.status().phase, JobPhase::Terminal(_)))
            && session
                .displaying
                .as_ref()
                .is_some_and(|displaying| displaying.job.status().cancellation_requested)
    })?;
    assert!(!fixture.session.retained.contains(&key("a")));

    drop(opened);
    fixture.await_display_terminal()?;
    // The display's terminal is the last wake this state receives, so one pass must both
    // retire the display and adopt the held load.
    fixture.session.poll(Duration::ZERO)?;
    assert!(fixture.session.failed.is_none());
    assert!(fixture.session.displaying.is_none());
    assert!(fixture.session.loading.is_none(), "the released load was left untaken");
    assert!(fixture.session.retained.contains(&key("a")));
    let _directory = fixture.finish()?;
    Ok(())
}
