// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the embedded canvas drawing device through the existing execution scope.

use super::*;
use sugarsubstitute_desktop::canvas::CanvasGraphics;

impl Session {
    /// Request the device only after an output is selected for the embedded surface.
    pub(crate) fn request_host_graphics(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Err("Canvas graphics admission is closed".into());
        }
        if self.host_graphics.is_none() && self.host_graphics_job.is_none() {
            self.host_graphics_failure = None;
            self.host_graphics_pending = true;
        }
        Ok(())
    }

    /// Borrow the completed device while the application retains its original reservation.
    pub(crate) fn host_graphics(&self) -> Option<&Arc<CanvasGraphics>> {
        self.host_graphics.as_ref()
    }

    /// Submit one bounded device creation without exposing scheduler handles to the desktop.
    pub(super) fn submit_host_graphics(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.host_graphics_pending || self.host_graphics_job.is_some() || self.closing {
            return Ok(());
        }
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            64 * 1024,
            Duration::from_secs(30),
            None,
        )?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            #[cfg(test)]
            let result = CanvasGraphics::fixture(&memory, &context);
            #[cfg(not(test))]
            let result = CanvasGraphics::prepare(&memory, &context);
            if result.is_ok() {
                tracing::info!(
                    operation = "canvas_graphics_ready",
                    job_id = context.job_id().get(),
                    "Canvas graphics preparation completed on its admitted worker"
                );
            }
            result.map_err(WorkError::new)
        }) {
            Ok(job) => {
                self.host_graphics_job = Some(job);
                self.host_graphics_pending = false;
            }
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Observe actual completion before making the device available to the native surface.
    pub(super) fn consume_host_graphics(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.host_graphics_job) else { return Ok(()) };
        match Self::terminal(outcome) {
            Ok(graphics) if self.host_graphics.is_none() => self.host_graphics = Some(graphics),
            Ok(graphics) => {
                self.retire(Retired { _host_graphics: Some(graphics), ..Default::default() })?;
                return Err("Canvas graphics device was already retained".into());
            }
            Err(cause) if self.closing => {
                tracing::debug!(operation="canvas_graphics_stopped", cause=%cause,
                    "Canvas graphics preparation reached a terminal outcome during close");
            }
            Err(cause) => {
                tracing::error!(operation="canvas_graphics_prepare", cause=%cause,
                    "Canvas graphics preparation failed");
                self.host_graphics_failure = Some(cause);
            }
        }
        Ok(())
    }
}
