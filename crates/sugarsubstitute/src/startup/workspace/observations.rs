// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route independently attributed native session observations before generic child handling.
use super::*;

impl WorkspaceWork {
    fn retire_native(&mut self, identity: MainIdentity, now: Moment) -> Result<(), Box<dyn Error>> {
        self.retired = true;
        #[cfg(feature = "native-composition")]
        self.deferred_surface.clear();
        self.media.retire(identity);
        self.capture = None;
        self.publish_input = None;
        self.pending = None;
        match &self.active {
            Some(Active::Encode { job, .. }) => job.cancel(),
            Some(Active::Decode { job, .. }) => job.cancel(),
            Some(Active::Publish { job, .. }) => job.cancel(),
            None => {
                if let Some(driver) = &mut self.driver {
                    driver.abandon_capture(now)?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::startup) fn observe(
        &mut self,
        observation: DesktopStartupObservation,
        now: Moment,
    ) -> Result<Option<DesktopStartupObservation>, Box<dyn Error>> {
        let identity = self.context.as_ref().map(|context| context.identity);
        match observation {
            #[cfg(feature = "native-composition")]
            DesktopStartupObservation::SurfaceMessage(observed) => self
                .hold_surface(observed)
                .map(|value| value.map(DesktopStartupObservation::SurfaceMessage)),
            DesktopStartupObservation::SessionMessage(observed) => {
                if Some(observed.identity()) != identity || self.retired {
                    return Err(
                        "Workspace message belongs to an unavailable native lifetime".into()
                    );
                }
                let (_, message, _lease, _claim) = observed.into_parts();
                let request = message.request_id.clone();
                let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                match driver.receive(message, now) {
                    Ok(Some(event)) if self.pending.is_none() && self.active.is_none() => {
                        self.pending = Some(event)
                    }
                    Ok(Some(_)) => {
                        return Err("Workspace worker request overlapped an owned operation".into());
                    }
                    Ok(None) => {}
                    Err(cause) => {
                        tracing::warn!(operation="workspace_session_ingress",request_id=%request,cause=%cause,
                            "Session operation refused; durable checkpoint retained");
                        driver.refused(request)?;
                    }
                }
                Ok(None)
            }
            DesktopStartupObservation::MainPlacementObserved(sample) => {
                if Some(sample.identity) != identity {
                    return Err("Workspace placement belongs to another native lifetime".into());
                }
                if sample.capture_id.is_some() && sample.capture_id != self.close_sample {
                    return Err("Workspace placement capture is not the owned final request".into());
                }
                self.driver.as_mut().ok_or("Workspace driver absent")?.observe_placement(
                    sample.sequence.get(),
                    sample.placement,
                    now,
                )?;
                if sample.capture_id.is_some() {
                    self.close_sample = None;
                }
                Ok(None)
            }
            DesktopStartupObservation::MainPlacementCaptureFailed {
                identity: observed,
                capture_id,
                cause,
            } => {
                if Some(observed) != identity
                    || capture_id.is_some() && capture_id != self.close_sample
                {
                    return Err("Workspace placement failure belongs to another capture".into());
                }
                tracing::warn!(operation="workspace_placement_capture",cause=%cause,
                    "Native placement unavailable; prior checked placement retained");
                if capture_id.is_some() {
                    self.close_sample = None;
                }
                Ok(None)
            }
            DesktopStartupObservation::SessionMediaRequested(request) => {
                self.media.offer(request)?;
                Ok(None)
            }
            DesktopStartupObservation::SessionMediaCancelled { identity, request_id } => {
                self.media.cancel(identity, request_id);
                Ok(None)
            }
            DesktopStartupObservation::MainRetired(observed) if Some(observed) == identity => {
                self.retire_native(observed, now)?;
                Ok(Some(DesktopStartupObservation::MainRetired(observed)))
            }
            DesktopStartupObservation::MainAbsent(observed) if Some(observed) == identity => {
                self.retire_native(observed, now)?;
                Ok(Some(DesktopStartupObservation::MainAbsent(observed)))
            }
            other => Ok(Some(other)),
        }
    }
}
