// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select one native device at startup, then attach the desktop peer on its original thread.
use super::*;
use sugarsubstitute_image_gpu::AdapterCatalog;

impl Session {
    pub(super) fn initialize_gpu(&mut self) -> Result<(), Box<dyn Error>> {
        if self.gpu_ready || self.device_job.is_some() || !self.device_pending {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let memory =
            self.resources.as_ref().ok_or("Canvas admission is unavailable")?.memory.clone();
        let key = self.options.gpu.ok_or("Canvas GPU selection is unavailable")?;
        let spec = self.spec(
            Operation::Infrastructure,
            JobClass::IoOrdinary,
            2 * 1024 * 1024,
            Duration::from_secs(30),
            None,
        )?;
        match runtime.client.submit_async(&runtime.scope, spec, move |context| async move {
            let catalog = AdapterCatalog::discover(&context).await?;
            let identity = catalog.resolve(key)?;
            catalog.select(identity, &memory, &context).await.map_err(WorkError::from)
        }) {
            Ok(job) => {
                self.device_job = Some(job);
                self.device_pending = false;
            }
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    pub(super) fn consume_device(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.device_job) else {
            return Ok(());
        };
        match Self::terminal(outcome) {
            Ok(device) if self.closing => {
                self.retire(Retired { _device: Some(device), ..Default::default() })?
            }
            Ok(device) => {
                let window = self
                    .native_target
                    .as_ref()
                    .map(|target| target.attach_gpu(device.clone()))
                    .transpose();
                match window {
                    Ok(window) => {
                        let resources =
                            self.resources.take().ok_or("Canvas resources are unavailable")?;
                        self.gpu_descriptor = Some(device.descriptor().clone());
                        self.resources = Some(resources.with_gpu(device, window));
                        self.gpu_ready = true;
                    }
                    Err(cause) => {
                        self.retire(Retired { _device: Some(device), ..Default::default() })?;
                        self.fail(cause);
                    }
                }
            }
            Err(cause) if self.closing => {
                tracing::debug!(operation="canvas_gpu_initialization_stopped", cause=%cause, "GPU initialization retired during close")
            }
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    /// The actual accepted backing generation, independent of a proof-only CPU snapshot.
    pub(super) fn current_frame(&self) -> Option<FrameStamp> {
        if self.options.gpu.is_some() {
            self.gpu_front.as_ref().and_then(GpuSurface::current)
        } else {
            self.front.as_ref().and_then(|surface| surface.current())
        }
    }
    /// Validate the exact mapping and authored identity committed by native acknowledgement.
    pub(super) fn displayed_coverage_is_valid(&self) -> Result<bool, Box<dyn Error>> {
        let Some(evidence) = self.display_evidence else {
            return Ok(true);
        };
        let Some(scene) = &self.scene else {
            return Ok(false);
        };
        let snapshot = scene.document.snapshot();
        Ok(evidence.document == (snapshot.identity(), snapshot.revision())
            && evidence.source_revision == scene.source_revision
            && self.display_source == Some(evidence.source)
            && self.display_view == Some(evidence.view)
            && (self.native_target.is_none() || self.native_presented == Some(evidence.target))
            && evidence.proves_complete(snapshot.extent())?)
    }
}
