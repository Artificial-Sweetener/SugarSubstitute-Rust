// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reconfigure native surfaces transactionally after admitted queue work reaches GPU retirement.

use crate::{
    GpuError,
    coordination::{ConfigurationAttempt, SubmissionPermit},
    surface_state::ReconfigureReason,
    window::{PendingConfiguration, WindowSurface, require_follow_up_after_configuration},
};
use std::time::Duration;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::Extent;

const PREFLIGHT_WAIT: Duration = Duration::from_millis(25);

fn settle_configuration<T>(
    current: &mut Option<T>,
    pending: &mut Option<T>,
    configured: Result<(), GpuError>,
) -> Result<(), GpuError> {
    match configured {
        Ok(()) => {
            let Some(replacement) = pending.take() else {
                return Err(GpuError::invalid(
                    "Native swapchain configuration lost its pending claim",
                ));
            };
            *current = Some(replacement);
            Ok(())
        }
        Err(cause) => {
            pending.take();
            Err(cause)
        }
    }
}

impl WindowSurface {
    pub(super) fn frame_submission(
        &mut self,
        extent: Extent,
        context: &JobContext,
    ) -> Result<Option<SubmissionPermit>, GpuError> {
        let reason = if self.extent.is_none() {
            Some(ReconfigureReason::Initial)
        } else if self.extent != Some(extent) {
            Some(ReconfigureReason::SizeChanged)
        } else {
            self.owner.reconfiguration.reason()
        };
        let Some(reason) = reason else {
            return self.owner.device.submission(context).map(Some);
        };
        if self.pending_configuration.as_ref().is_some_and(|pending| pending.extent != extent) {
            self.pending_configuration.take();
        }
        if self.pending_configuration.is_none() {
            let Some(intent) = self.owner.device.submission.request_configuration() else {
                return Ok(None);
            };
            self.pending_configuration = Some(PendingConfiguration { extent, reason, intent });
        }
        let pending = self
            .pending_configuration
            .take()
            .ok_or_else(|| GpuError::invalid("Native swapchain lost configuration custody"))?;
        let PendingConfiguration { extent: pending_extent, reason: pending_reason, intent } =
            pending;
        match intent.try_acquire() {
            ConfigurationAttempt::Deferred(intent) => {
                self.pending_configuration = Some(PendingConfiguration {
                    extent: pending_extent,
                    reason: pending_reason,
                    intent,
                });
                Ok(None)
            }
            ConfigurationAttempt::Ready(exclusive) => {
                match self.owner.device.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(PREFLIGHT_WAIT),
                }) {
                    Ok(status) if status.wait_finished() => {}
                    Ok(_) | Err(wgpu::PollError::Timeout) => {
                        crate::adapter::check(context)?;
                        self.pending_configuration = Some(PendingConfiguration {
                            extent: pending_extent,
                            reason: pending_reason,
                            intent: exclusive.defer(),
                        });
                        return Ok(None);
                    }
                    Err(cause @ wgpu::PollError::WrongSubmissionIndex(..)) => {
                        return Err(GpuError::new("gpu_surface_configuration_preflight", cause));
                    }
                }
                let bytes = (u64::from(crate::display::row_bytes(extent.width()))
                    * u64::from(extent.height())
                    * 3)
                .next_multiple_of(65536)
                    + 65536;
                let memory = self
                    .owner
                    .device
                    .memory
                    .claim(bytes)
                    .map_err(|cause| GpuError::new("gpu_swapchain_admission", cause))?;
                {
                    let mut claims =
                        self.owner.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if claims.pending.is_some() {
                        return Err(GpuError::invalid(
                            "Native swapchain configuration did not retire",
                        ));
                    }
                    claims.pending = Some(memory);
                }
                let configured = self.owner.device.faults.configure(|| {
                    self.owner.surface.configure(
                        &self.owner.device.device,
                        &wgpu::SurfaceConfiguration {
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | if self.owner.capture {
                                    wgpu::TextureUsages::COPY_SRC
                                } else {
                                    wgpu::TextureUsages::empty()
                                },
                            format: self.owner.format,
                            color_space: wgpu::SurfaceColorSpace::Srgb,
                            width: extent.width(),
                            height: extent.height(),
                            desired_maximum_frame_latency: 1,
                            present_mode: wgpu::PresentMode::Fifo,
                            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                            view_formats: vec![],
                        },
                    );
                });
                let settled = {
                    let mut claims =
                        self.owner.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let claims = &mut *claims;
                    settle_configuration(&mut claims.current, &mut claims.pending, configured)
                };
                if let Err(cause) = settled {
                    if cause.is_surface_configuration_busy() {
                        self.pending_configuration = Some(PendingConfiguration {
                            extent: pending_extent,
                            reason: pending_reason,
                            intent: exclusive.defer(),
                        });
                        tracing::debug!(operation="image_gpu_surface_configuration_deferred",
                            reason=pending_reason.name(), width=extent.width(),
                            height=extent.height(), cause=%cause,
                            "Deferred native GPU surface configuration until presentation retires");
                        return Ok(None);
                    }
                    tracing::warn!(operation="image_gpu_surface_configuration",
                        reason=pending_reason.name(), width=extent.width(), height=extent.height(),
                        cause=%cause, "Native GPU surface configuration was rejected");
                    return Err(cause);
                }
                self.extent = Some(extent);
                self.owner.reconfiguration.configured();
                require_follow_up_after_configuration(&self.owner.follow_up_after_configuration);
                tracing::info!(
                    operation = "image_gpu_surface_configuration",
                    reason = pending_reason.name(),
                    width = extent.width(),
                    height = extent.height(),
                    "Configured native GPU surface after queue retirement"
                );
                Ok(Some(exclusive.downgrade()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_configuration_discards_only_the_pending_claim() {
        let mut current = Some(7_u8);
        let mut pending = Some(11_u8);
        assert!(
            settle_configuration(
                &mut current,
                &mut pending,
                Err(GpuError::invalid("Rejected test configuration")),
            )
            .is_err()
        );
        assert_eq!(current, Some(7));
        assert_eq!(pending, None);

        pending = Some(13);
        assert!(settle_configuration(&mut current, &mut pending, Ok(())).is_ok());
        assert_eq!(current, Some(13));
        assert_eq!(pending, None);
    }
}
