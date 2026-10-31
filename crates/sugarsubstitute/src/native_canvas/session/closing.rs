// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence canvas input and retain every cancellation outcome through the original cutoff.

use super::*;
use std::fmt;
use sugarsubstitute_canvas::ToolCancelReason;
use sugarsubstitute_canvas_render::PresentationStage;

/// Keep both independent close failures when pacing and tool cancellation both fail.
#[derive(Debug)]
struct CloseFailures {
    pacing: Box<dyn Error>,
    tools: Box<dyn Error>,
}
impl fmt::Display for CloseFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Canvas pacing and tool cancellation failed")
    }
}
impl Error for CloseFailures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.pacing.as_ref())
    }
}
impl Drop for CloseFailures {
    fn drop(&mut self) {
        tracing::error!(operation="canvas_close", cause=%self.tools,
            "Additional canvas tool cancellation failure");
    }
}

impl Session {
    /// Adopt the composing application's original cutoff before transferring external preview custody.
    /// This only tightens disposal metadata; the desktop closes the session after its raw stage moves.
    pub(crate) fn adopt_preview_retirement_cutoff(&mut self, cutoff: Moment) -> Moment {
        let cutoff = self.shutdown_deadline.map_or(cutoff, |original| original.min(cutoff));
        self.shutdown_deadline = Some(cutoff);
        self.retirement_deadline = self.retirement_deadline.map(|deadline| deadline.min(cutoff));
        self.deferred_retirement_deadline =
            self.deferred_retirement_deadline.map(|deadline| deadline.min(cutoff));
        if self.pending_retirement.is_some() && self.retirement_deadline.is_none() {
            self.retirement_deadline = Some(cutoff);
        }
        if self.deferred_retirement.is_some() && self.deferred_retirement_deadline.is_none() {
            self.deferred_retirement_deadline = Some(cutoff);
        }
        cutoff
    }

    /// Stop canvas work under the composing owner's original, nonrenewable cutoff.
    /// Repeated requests may tighten this deadline but cannot grant more time.
    /// The cutoff is in the injected execution clock's epoch.
    ///
    /// # Errors
    /// Preserves pacing and tool-cancellation failures while retaining outstanding jobs.
    pub(crate) fn close_until(&mut self, cutoff: Moment) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        let first_close = !self.closing;
        let cutoff = self.adopt_preview_retirement_cutoff(cutoff);
        if self.shutdown_started
            && let Some(runtime) = &mut self.runtime
        {
            runtime.request_shutdown(cutoff);
        }
        if self.shutdown_started
            && let Some(owner) = &mut self.starting_owner
        {
            owner.request_shutdown(cutoff);
        }
        self.closing = true;
        #[cfg(feature = "native-composition")]
        self.withdraw_preview()?;
        #[cfg(feature = "native-composition")]
        self.close_acquirer_retirement(cutoff, first_close);
        self.initialization_pending = false;
        self.device_pending = false;
        #[cfg(all(windows, feature = "native-composition"))]
        {
            self.host_graphics_pending = false;
            if let Some(job) = &self.host_graphics_job {
                job.cancel();
            }
        }
        self.presentation = attachment::Presentation::Headless;
        self.picker = false;
        #[cfg(feature = "native-composition")]
        if let Some(Open { source: OpenSource::Acquired(owner), .. }) = &self.pending_open {
            owner.cancel();
        }
        if self
            .pending_open
            .as_ref()
            .is_some_and(|pending| !pending.source.requires_worker_retirement())
        {
            self.pending_open = None;
        }
        let now = self.navigation_time();
        let pacing = self
            .pacing
            .readiness(now)
            .and_then(|readiness| match readiness {
                PresentationReadiness::Busy {
                    target,
                    stage: PresentationStage::AwaitingNative,
                } => self.pacing.abandoned(target, now),
                _ => Ok(()),
            })
            .map_err(Box::<dyn Error>::from);
        self.document.cpu_pending = None;
        self.awaiting_gpu = None;
        self.document.pending_cpu_evidence = None;
        self.document.pending_gpu_evidence = None;
        self.document.projection_base = None;
        let tools = self
            .document
            .tools
            .cancel(
                ToolCancelReason::Shutdown,
                self.document.scene.as_ref().map(|scene| &scene.document),
                now,
            )
            .map_err(Box::<dyn Error>::from);
        if let Some(job) = &self.initializing {
            job.cancel();
        }
        if let Some(job) = &self.device_job {
            job.cancel();
        }
        if let Some(job) = &self.loading {
            #[cfg(feature = "native-composition")]
            if let Some(owner) = &job.acquirer {
                owner.cancel();
            }
            job.job.cancel();
        }
        if let Some(rendering) = &self.rendering {
            rendering.job.cancel();
        }
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        match (pacing, tools) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
            (Err(pacing), Err(tools)) => Err(Box::new(CloseFailures { pacing, tools })),
        }
    }
}
