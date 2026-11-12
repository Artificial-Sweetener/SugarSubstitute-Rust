// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select one native device at startup, then attach the desktop peer on its original thread.
use super::*;
use crate::native_canvas::gpu_policy::{GpuRequest, select_embedded_gpu};
use sugarsubstitute_image_gpu::AdapterCatalog;

impl Session {
    pub(super) fn initialize_gpu(&mut self) -> Result<(), Box<dyn Error>> {
        if self.gpu_ready || self.device_job.is_some() || !self.device_pending || !self.gpu_admitted
        {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let memory =
            self.resources.as_ref().ok_or("Canvas admission is unavailable")?.memory.clone();
        let request = self.config.gpu.ok_or("Canvas GPU selection is unavailable")?;
        let spec = self.event_spec(
            Operation::Infrastructure,
            JobClass::IoOrdinary,
            2 * 1024 * 1024,
            None,
        )?;
        match runtime.client.submit_async(&runtime.scope, spec, move |context| async move {
            let catalog = AdapterCatalog::discover(&context).await?;
            match request {
                GpuRequest::Required(key) => {
                    let identity = catalog.resolve(key)?;
                    Ok(Some(catalog.select(identity, &memory, &context).await?))
                }
                GpuRequest::Embedded(policy) => {
                    let Some(identity) = select_embedded_gpu(catalog.adapters(), policy) else {
                        return Ok(None);
                    };
                    match catalog.select(identity, &memory, &context).await {
                        Ok(device) => Ok(Some(device)),
                        Err(cause) => {
                            tracing::warn!(operation = "canvas_gpu_initialization",
                                job_id = context.job_id().get(), cause_chain = %crate::diagnostics::CauseChain(&cause),
                                "Embedded canvas GPU initialization failed");
                            Ok(None)
                        }
                    }
                }
            }
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
            Ok(Some(device)) if self.closing => {
                self.retire(Retired { _device: Some(device), ..Default::default() })?
            }
            Ok(None) if self.closing => {}
            // Only an embedded request yields no device; a required selector fails instead.
            Ok(None) => self.fall_back_to_cpu("adapter_unavailable", ""),
            #[cfg(all(windows, feature = "native-composition"))]
            Ok(Some(device)) if self.presentation.composition() => {
                // The composition device must share this adapter before any display work.
                self.composition_device = Some(device);
                self.request_host_graphics()?;
            }
            Ok(Some(device)) => {
                let window = self
                    .presentation
                    .gpu_window()
                    .map(|target| target.attach_gpu(device.clone()))
                    .transpose();
                match window {
                    Ok(window) => {
                        let resources =
                            self.resources.take().ok_or("Canvas resources are unavailable")?;
                        self.gpu_descriptor = Some(device.descriptor().clone());
                        self.gpu_diagnostics = Some(device.diagnostics());
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
        if self.gpu_document() {
            self.document.gpu_front.as_ref().and_then(GpuSurface::current)
        } else {
            self.document.front.as_ref().and_then(|surface| surface.current())
        }
    }
    /// Validate the exact mapping and authored identity committed by native acknowledgement.
    pub(super) fn displayed_coverage_is_valid(&self) -> Result<bool, Box<dyn Error>> {
        let Some(evidence) = self.document.display_evidence else {
            return Ok(true);
        };
        let Some((_, snapshot)) = self.document.shown() else {
            return Ok(false);
        };
        Ok(self.display_shows_current_source()
            && self.document.display_source == Some(evidence.source)
            && self.document.display_view == Some(evidence.view)
            && evidence.proves_complete(snapshot.extent())?)
    }
}
