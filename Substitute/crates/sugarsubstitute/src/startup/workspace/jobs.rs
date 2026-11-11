// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe every owned worker terminal before admitting its successor.
use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl WorkspaceWork {
    pub(super) fn advance(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if self.pending_open.is_some()
            && self.load.is_none()
            && self.active.is_none()
            && self.pending.is_none()
            && self.publish_input.is_none()
            && self.capture.is_none()
            && self.retirement_job.is_none()
            && self.media.settled()
            && self.retirements.try_lock().is_ok_and(|owners| owners.is_empty())
        {
            let (context, effect) = self.pending_open.take().ok_or("Workspace open disappeared")?;
            self.open(context, effect)?;
        }
        if let Some(job) = &mut self.load
            && let Some(completion) = job.try_take_completion()
        {
            self.load = None;
            match completion.outcome {
                JobOutcome::Success(loaded) if self.closing.is_none() => {
                    self.driver = Some(WorkspaceDriver::new(
                        loaded.profile.scope.clone(),
                        loaded.bootstrap.origin_text(),
                        loaded.bootstrap.webview().to_owned(),
                        loaded.restored,
                        loaded.driver_memory,
                    )?);
                    self.input_directory =
                        Some(loaded.profile.workspace.directory().join("input-canvas"));
                    self.store = Some(loaded.profile.workspace.clone());
                    self.profile = Some(Arc::new(Mutex::new(loaded.profile)));
                    self.delivery = Some(loaded.delivery_memory);
                    self.input_plan = loaded.input_plan;
                    self.input_association = None;
                    self.bootstrap = Some(loaded.bootstrap);
                    self.ready = true;
                }
                JobOutcome::Success(_) | JobOutcome::Cancelled | JobOutcome::TimedOut
                    if self.closing.is_some() => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                outcome => {
                    return Err(std::io::Error::other(format!(
                        "Workspace load stopped: {:?}",
                        outcome.kind()
                    ))
                    .into());
                }
            }
        }
        self.observe_active(now)?;
        if self.closing.is_some_and(|cutoff| now >= cutoff)
            && let Some(context) = &self.context
        {
            self.media.retire(context.identity);
        }
        self.media.advance(
            media::MediaAdmission {
                client: self.client.as_ref().ok_or("Workspace execution absent")?,
                scope: self.scope.as_ref().ok_or("Workspace scope absent")?,
                jobs: &self.jobs,
                now,
                cutoff: self.retirement_until,
            },
            media::MediaSource {
                driver: self.driver.as_ref(),
                store: self.store.as_ref(),
                resident: self.delivery.as_ref(),
                identity: self.context.as_ref().map(|context| context.identity),
            },
            Instant::now(),
            &mut self.messages,
        )?;
        if !self.retired
            && let Some(driver) = &mut self.driver
        {
            driver.tick(now)?;
        }
        if self.active.is_none() {
            self.submit_pending(now)?;
        }
        if self.closing.is_some_and(|cutoff| now >= cutoff) {
            if let Some(job) = &self.load {
                job.cancel();
            }
            match &self.active {
                Some(Active::Encode { job, .. }) => job.cancel(),
                Some(Active::Decode { job, .. }) => job.cancel(),
                Some(Active::Publish { job, .. }) => job.cancel(),
                None => {}
            }
            self.pending = None;
            self.capture = None;
            self.publish_input = None;
        }
        Ok(())
    }
    fn observe_active(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        let Some(active) = &mut self.active else {
            return Ok(());
        };
        match active {
            Active::Encode { request, transfer, job } => {
                let owned = self.driver.as_ref().is_some_and(|driver| driver.owns_restore(request));
                if !owned {
                    job.cancel();
                }
                let Some(completion) = job.try_take_completion() else {
                    return Ok(());
                };
                let (request, transfer) = (request.clone(), transfer.clone());
                self.active = None;
                if !owned || self.retired {
                    self.driver
                        .as_mut()
                        .ok_or("Workspace driver absent")?
                        .encoding_stopped(&request, &transfer)?;
                    if let JobOutcome::Failure(cause) = completion.outcome {
                        tracing::warn!(operation="workspace_encoding_retirement",cause_chain=%crate::diagnostics::CauseChain(&cause),"Superseded encoding stopped; durable checkpoint retained");
                    }
                    return Ok(());
                }
                match completion.outcome {
                    JobOutcome::Success(encoded) => self
                        .driver
                        .as_mut()
                        .ok_or("Workspace driver absent")?
                        .encoded_restore(&request, &transfer, encoded)?,
                    JobOutcome::Failure(cause) => {
                        tracing::warn!(operation="workspace_encode",cause_chain=%crate::diagnostics::CauseChain(&cause),"Workspace replay failed; original checkpoint protected");
                        let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                        driver.encoding_stopped(&request, &transfer)?;
                        driver.restore_failed(&request, &RestoreFailure::Encoding)?;
                    }
                    outcome => {
                        tracing::warn!(operation="workspace_encode",outcome=?outcome.kind(),"Workspace replay stopped; original checkpoint protected");
                        let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                        driver.encoding_stopped(&request, &transfer)?;
                        driver.restore_failed(&request, &RestoreFailure::Encoding)?;
                    }
                }
            }
            Active::Decode { request, ticket, job } => {
                let Some(completion) = job.try_take_completion() else {
                    return Ok(());
                };
                let (request, ticket) = (request.clone(), ticket.clone());
                self.active = None;
                match completion.outcome {
                    JobOutcome::Success(_) if self.retired => self
                        .driver
                        .as_mut()
                        .ok_or("Workspace driver absent")?
                        .abandon_capture(now)?,
                    JobOutcome::Success(Decoded { mut captured, association }) => {
                        let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                        match driver.decoded_capture(&request, &ticket, &mut captured, now) {
                            Ok(()) => {
                                self.input_association = association;
                                self.capture = Some((ticket, captured));
                            }
                            Err(StoreError::Contract(
                                sugarsubstitute_contracts::desktop_session::SessionError::Stale,
                            )) => {}
                            Err(cause) => return Err(cause.into()),
                        }
                    }
                    JobOutcome::Failure(cause) => {
                        tracing::warn!(operation="workspace_capture_decode",cause_chain=%crate::diagnostics::CauseChain(&cause),"Workspace capture rejected; durable checkpoint retained");
                        let protected = matches!(cause, StoreError::Protected);
                        let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                        driver.publication_failed(&ticket, now)?;
                        if protected {
                            driver.protect_originals();
                        }
                    }
                    outcome => {
                        tracing::warn!(operation="workspace_capture_decode",outcome=?outcome.kind(),"Workspace capture stopped; durable checkpoint retained");
                        self.driver
                            .as_mut()
                            .ok_or("Workspace driver absent")?
                            .publication_failed(&ticket, now)?;
                    }
                }
            }
            Active::Publish { ticket, job } => {
                let Some(completion) = job.try_take_completion() else {
                    return Ok(());
                };
                let ticket = ticket.clone();
                self.active = None;
                let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
                match completion.outcome {
                    JobOutcome::Success(published) => {
                        driver.published(&ticket, published.captured, &published.receipt, now)?
                    }
                    JobOutcome::Failure(cause) => {
                        tracing::warn!(operation="workspace_publish",cause_chain=%crate::diagnostics::CauseChain(&cause),"Workspace publication failed; previous checkpoint retained");
                        driver.publication_failed(&ticket, now)?;
                    }
                    outcome => {
                        tracing::warn!(operation="workspace_publish",outcome=?outcome.kind(),"Workspace publication stopped; previous checkpoint retained");
                        driver.publication_failed(&ticket, now)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn submit_pending(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if let Some(DriverEvent::EncodeRestore { request_id, transfer_id, .. }) = &self.pending
            && !self.driver.as_ref().is_some_and(|driver| driver.owns_restore(request_id))
        {
            self.driver
                .as_mut()
                .ok_or("Workspace driver absent")?
                .encoding_stopped(request_id, transfer_id)?;
            self.pending = None;
        }
        let Some(event) = self.pending.as_ref() else {
            return Ok(());
        };
        // Encoding a restore and decoding a capture are local work that ends on its observed
        // outcome; only the shutdown cutoff, once a close began, bounds them.
        let until = self.driver.as_ref().and_then(WorkspaceDriver::shutdown_cutoff);
        if until.is_some_and(|until| now >= until) {
            return Ok(());
        }
        let spec = self.job(WorkspaceStore::reservation_bytes(), until)?;
        let client = self.client.as_ref().ok_or("Workspace execution absent")?;
        let scope = self.scope.as_ref().ok_or("Workspace scope absent")?;
        let request = match event {
            DriverEvent::EncodeRestore { request_id, .. }
            | DriverEvent::DecodeCapture { request_id, .. } => request_id.clone(),
        };
        let result = match event {
            DriverEvent::EncodeRestore { transfer_id, .. } => {
                let transfer = transfer_id.clone();
                client
                    .submit_blocking_owned_input(
                        scope,
                        spec,
                        &mut self.pending,
                        move |event, context| {
                            let DriverEvent::EncodeRestore { restored, .. } = event else {
                                return Err(StoreError::Changed);
                            };
                            let checkpoint =
                                &restored.checkpoint.as_ref().ok_or(StoreError::Changed)?.1;
                            workspace_session::EncodedMaterialization::encode(checkpoint, &context)
                        },
                    )
                    .map(|job| Active::Encode { request, transfer, job })
            }
            DriverEvent::DecodeCapture { ticket, .. } => {
                let ticket = ticket.clone();
                let legacy = matches!(
                    self.input_plan,
                    crate::input_documents::InputRestorePlan::AwaitingAssociation
                );
                let user = self.user.clone();
                let directory =
                    self.input_directory.clone().ok_or("Workspace input namespace absent")?;
                client
                    .submit_blocking_owned_input(
                        scope,
                        spec,
                        &mut self.pending,
                        move |event, context| {
                            let DriverEvent::DecodeCapture { transfer, transfer_id, .. } = event
                            else {
                                return Err(StoreError::Changed);
                            };
                            let captured =
                                transfer.finish(&transfer_id, context.now()).map_err(|cause| {
                                    StoreError::Storage(std::io::Error::other(cause))
                                })?;
                            let association = if legacy {
                                Some(
                                    crate::input_documents::legacy::adopt(
                                        &user,
                                        &directory,
                                        &captured.checkpoint,
                                        &context,
                                    )
                                    .map_err(|cause| {
                                        let protected = matches!(&cause, crate::input_documents::StoreError::Unverified)
                                            || matches!(&cause, crate::input_documents::StoreError::Storage(error) if error.kind() == std::io::ErrorKind::InvalidData);
                                        if protected {
                                            tracing::warn!(operation="workspace_legacy_input_association",cause=%cause,"Native association incompatible; originals protected");
                                            StoreError::Protected
                                        } else { StoreError::Storage(std::io::Error::other(cause)) }
                                    })?,
                                )
                            } else {
                                None
                            };
                            Ok(Decoded { captured, association })
                        },
                    )
                    .map(|job| Active::Decode { request, ticket, job })
            }
        };
        match result {
            Ok(active) => self.active = Some(active),
            Err(OwnedInputError::Admission(
                AdmissionError::MemorySaturated { .. } | AdmissionError::QueueSaturated { .. },
            )) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
