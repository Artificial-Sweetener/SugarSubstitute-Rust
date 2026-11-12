// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide each surface-test wait from the deterministic test boundary instead of elapsed time.
//!
//! A canvas under test can make progress without new host input only three ways: an admitted job
//! runs or retires worker-held custody and then publishes a wake, a terminal it published has not
//! yet been offered to the canvas, or the canvas reported a timer that the injected clock can be
//! advanced to. When none of these holds and the awaited answer has not arrived, nothing can ever
//! deliver it, so the wait fails at once with what it was waiting for. The completion wake's own
//! process-wide stall watch remains only as the last-resort backstop for a worker that never
//! returns.

use super::*;

/// Injected display time advanced when the canvas reports a timer, matching one fixture turn.
const TIMER_STEP_NANOS: u64 = 100_000_000;

/// Execution progress observed between host-loop turns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Execution {
    /// Admitted work is queued, running, or retiring custody its worker still holds after
    /// acknowledgement; its completion or retirement supplies a wake.
    Running,
    /// A terminal completion has not been taken by its owner; its wake may still be in flight.
    Delivering,
    /// No admitted work remains, so only host input or a reported deadline can make progress.
    Idle,
}

/// Classify the admitted work of `owner` that can still publish a completion wake.
pub(super) fn classify(owner: &ExecutionOwner) -> Result<Execution, Box<dyn Error>> {
    let progress = owner.poll_shutdown()?;
    if progress.running + progress.queued > 0 {
        return Ok(Execution::Running);
    }
    // An acknowledged record stays registered only while cancellation or native custody defers
    // its retirement, and the worker that retires it publishes a capacity wake.
    if progress.unresolved_jobs.iter().any(|job| job.acknowledged) {
        return Ok(Execution::Running);
    }
    let delivering = progress.unresolved_jobs.iter().any(|job| {
        matches!(job.phase, sugarsubstitute_contracts::execution::JobPhase::Terminal(_))
    });
    Ok(if delivering { Execution::Delivering } else { Execution::Idle })
}

/// The execution owner, completion wake and injected clock one canvas fixture runs on.
#[derive(Clone, Copy)]
pub(super) struct Boundary<'a> {
    pub(super) client: &'a ExecutionClient,
    pub(super) owner: &'a ExecutionOwner,
    pub(super) wake: &'a Wake,
    pub(super) clock: &'a ClockValue,
}

impl Boundary<'_> {
    /// Deliver completions and poll `canvas` once, as one host-loop turn does.
    pub(super) fn pump(&self, canvas: &mut EmbeddedCanvas) -> Result<(), Box<dyn Error>> {
        pump(canvas, self.client)
    }

    /// Make the next progress after a turn that snapshotted wake `observed`, then delivered
    /// completions and polled `canvas` without finding what it awaits: return at once for a newer
    /// wake or a terminal not yet offered to the canvas, block for running work's wake, or advance
    /// the injected clock toward the timer `canvas` reports.
    ///
    /// # Errors
    /// Fails immediately, naming `awaiting`, when no job is runnable, every terminal was already
    /// offered to the canvas, and the canvas reports no timer, because nothing can then deliver
    /// what the test awaits.
    pub(super) fn progress(
        &self,
        canvas: &EmbeddedCanvas,
        observed: u64,
        awaiting: &str,
    ) -> Result<(), Box<dyn Error>> {
        if self.wake.revision() != observed {
            return Ok(());
        }
        let execution = classify(self.owner)?;
        if execution == Execution::Running {
            self.wake.wait_after(observed);
        } else if canvas.next_deadline().is_some() {
            self.clock.0.fetch_add(TIMER_STEP_NANOS, Ordering::SeqCst);
        } else if execution == Execution::Idle || !self.newly_terminal()? {
            // A terminal's notice is enqueued under the same lock that makes it observable, so an
            // empty mailbox proves the canvas already declined every untaken terminal, and the
            // wake that may still follow it would only repeat that turn.
            return Err(self.quiescent(canvas, observed, awaiting).into());
        }
        // Otherwise the caller's next turn offers the newly terminal job to the canvas.
        Ok(())
    }

    /// Whether a terminal notice arrived after the caller's last delivery, so its canvas has not
    /// yet been offered it.
    fn newly_terminal(&self) -> Result<bool, Box<dyn Error>> {
        let batch = self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(!batch.notifications.is_empty())
    }

    /// Describe a quiescent canvas that still owes `awaiting`.
    fn quiescent(&self, canvas: &EmbeddedCanvas, observed: u64, awaiting: &str) -> String {
        let jobs = self.owner.poll_shutdown().map_or_else(
            |cause| format!("unobservable: {cause}"),
            |progress| format!("{:?}", progress.unresolved_jobs),
        );
        format!(
            "Deadlocked waiting for {awaiting}: no job is runnable, the canvas declined every \
             untaken completion and reports no timer (wake revision {observed}, injected clock \
             {}ns); unresolved jobs {jobs}; preparation {:?}",
            self.clock.0.load(Ordering::SeqCst),
            canvas.session.as_ref().map(Session::native_preparation_snapshot),
        )
    }

    /// Turn `canvas` until `reached` yields a value, making boundary progress between turns.
    ///
    /// # Errors
    /// Returns the first failure of `reached`, a quiescent canvas that can no longer reach
    /// `awaiting`, or a canvas or execution failure.
    pub(super) fn until<T>(
        &self,
        canvas: &mut EmbeddedCanvas,
        awaiting: &str,
        mut reached: impl FnMut(&mut EmbeddedCanvas) -> Result<Option<T>, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.pump(canvas)?;
            if let Some(value) = reached(canvas)? {
                return Ok(value);
            }
            self.progress(canvas, observed, awaiting)?;
        }
    }

    /// Turn a started canvas until it reports ready.
    ///
    /// # Errors
    /// Returns a quiescent canvas that never became ready, or a canvas or execution failure.
    pub(super) fn wait_ready(&self, canvas: &mut EmbeddedCanvas) -> Result<(), Box<dyn Error>> {
        self.until(canvas, "the started canvas to report ready", |canvas| {
            Ok(canvas.reported_ready.then_some(()))
        })
    }

    /// Turn the canvas until it answers `id`, requiring `status`; `awaiting` names the request.
    ///
    /// # Errors
    /// Returns a mismatched answer, a quiescent canvas that can no longer answer, or a canvas
    /// or execution failure.
    pub(super) fn wait_reply(
        &self,
        canvas: &mut EmbeddedCanvas,
        id: u64,
        status: &str,
        awaiting: &str,
    ) -> Result<(), Box<dyn Error>> {
        let command = self.until(canvas, awaiting, |canvas| Ok(canvas.take_command()))?;
        check_reply(command, id, status)
    }

    /// Deliver completions until a closed canvas has settled every owned job.
    ///
    /// # Errors
    /// Returns a quiescent canvas that can no longer settle, or a canvas or execution failure.
    pub(super) fn settle_closed(&self, canvas: &mut EmbeddedCanvas) -> Result<(), Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if canvas.settled()? {
                return Ok(());
            }
            self.progress(canvas, observed, "the closed canvas to settle its jobs")?;
        }
    }
}
