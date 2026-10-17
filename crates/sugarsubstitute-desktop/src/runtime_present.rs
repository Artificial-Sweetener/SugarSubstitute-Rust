// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure native submissions and optional source evidence independently of preparation.

use super::{Presenter, observe_move};
use crate::{DesktopError, DesktopReceipt, NativeMaterial, WindowContent, frame_audit::FrameAudit};
use std::time::{Duration, Instant};
use sugarsubstitute_contracts::presentation::{FrameIdentity, PreparedFrame};
use tao::window::Window;

/// Retain native timing and at most two explicitly audited prepared allocations.
pub(crate) struct Presentations {
    started: Instant,
    first: Option<Duration>,
    count: u64,
    prepared_count: u64,
    expose_count: u64,
    last_identity: Option<FrameIdentity>,
    work: Duration,
    intervals: Duration,
    longest: Duration,
    last: Option<Instant>,
    audit: Option<FrameAudit>,
}

impl Presentations {
    /// Start measurements without preparing pixels or starting a scheduler.
    pub(crate) fn new(started: Instant, audit: bool) -> Self {
        Self {
            started,
            first: None,
            count: 0,
            prepared_count: 0,
            expose_count: 0,
            last_identity: None,
            work: Duration::ZERO,
            intervals: Duration::ZERO,
            longest: Duration::ZERO,
            last: None,
            audit: audit.then(FrameAudit::default),
        }
    }

    /// Submit validated storage and paint native controls; never scan alpha or rasterize.
    pub(crate) fn submit(
        &mut self,
        window: &Window,
        presenter: &mut Presenter,
        frame: &PreparedFrame,
        timings: Option<&crate::timing::TimingObserver>,
        content: &impl WindowContent,
        #[cfg(target_os = "windows")] controls: &mut crate::windows_controls::Controls,
    ) -> Result<(), DesktopError> {
        crate::timing::observe_submission(timings, frame.identity(), || {
            let began = Instant::now();
            let result = (|| {
                let before = observe_move(window, self.audit.is_some())?;
                #[cfg(target_os = "windows")]
                let console = content.console();
                #[cfg(target_os = "windows")]
                {
                    let excluded = console
                        .as_ref()
                        .filter(|view| view.visible)
                        .map(|view| {
                            crate::control_geometry::physical_bounds(
                                &view.native_control(),
                                window.scale_factor(),
                            )
                        })
                        .transpose()?;
                    presenter.present(frame, excluded)?;
                }
                #[cfg(not(target_os = "windows"))]
                presenter.present(frame)?;
                #[cfg(target_os = "windows")]
                {
                    controls.update(content.accessible_controls())?;
                    controls.present_console(frame, console)?;
                    controls.draw_focus()?;
                }
                #[cfg(not(target_os = "windows"))]
                let _ = content;
                let after = observe_move(window, self.audit.is_some())?;
                if let Some(audit) = &mut self.audit {
                    audit.record(
                        Ok(frame.clone()),
                        (before && after).then(|| self.started.elapsed()),
                    )?;
                }
                Ok(())
            })();
            self.record_submission(result, frame.identity(), began, Instant::now())
        })
    }

    /// Count only successful native/control submissions, classifying reuse against actual delivery.
    fn record_submission(
        &mut self,
        result: Result<(), DesktopError>,
        identity: FrameIdentity,
        began: Instant,
        submitted: Instant,
    ) -> Result<(), DesktopError> {
        result?;
        if self.last_identity == Some(identity) {
            self.expose_count += 1;
        } else {
            self.prepared_count += 1;
        }
        self.last_identity = Some(identity);
        self.work += submitted.duration_since(began);
        if let Some(previous) = self.last {
            let interval = submitted.duration_since(previous);
            self.intervals += interval;
            self.longest = self.longest.max(interval);
        }
        self.last = Some(submitted);
        if self.first.is_none() {
            let elapsed = submitted.duration_since(self.started);
            tracing::info!(
                operation = "native_first_frame",
                elapsed_us = elapsed.as_micros(),
                "Submitted the first prepared frame"
            );
            self.first = Some(elapsed);
        }
        self.count += 1;
        Ok(())
    }

    /// Transfer guarded audit results only after native teardown finishes.
    pub(crate) fn finish(self, material: NativeMaterial) -> Result<DesktopReceipt, DesktopError> {
        Ok(DesktopReceipt {
            first_frame: self.first.ok_or_else(|| {
                DesktopError::message("present_frame", "window closed before the first frame")
            })?,
            frames_presented: self.count,
            prepared_frames_presented: self.prepared_count,
            retained_frame_exposes: self.expose_count,
            material,
            total_frame_work: self.work,
            total_frame_intervals: self.intervals,
            longest_frame_interval: self.longest,
            frame_audit: self.audit.and_then(FrameAudit::finish),
            timings: None,
        })
    }
}

#[cfg(test)]
#[path = "runtime_present_tests.rs"]
mod tests;
