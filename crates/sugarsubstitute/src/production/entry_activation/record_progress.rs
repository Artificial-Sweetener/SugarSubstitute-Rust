// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt each retained desktop's actual attachment, profile and lifecycle owners.
use super::progress::with_parent;
use super::*;
impl ProductionActivation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn progress_record(
        record: &mut Record,
        incidents: &mut incident_flow::IncidentFlow,
        capture: &mut CaptureRegistration,
        backend: &mut CoordinatorBackend,
        executable: &std::path::Path,
        working: &std::path::Path,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
        closing: bool,
    ) -> Result<()> {
        let admission_closed = closing
            || record.failed.is_some()
            || record.pending_desktop_failure.is_some()
            || record.restart.installation_pending;
        if !incidents.host.prepared() && !admission_closed {
            return Ok(());
        }
        if let Some(completion) = &record.projected {
            match &completion.outcome {
                JobOutcome::Success(value) if !admission_closed => {
                    capture.start(&value.capture_root, value.until, resources)?
                }
                JobOutcome::Success(_) => {}
                _ => {
                    return Err(
                        "Actual entry projection failed with retained original outcome".into()
                    );
                }
            }
        }
        if !admission_closed
            && record.attach.is_none()
            && record.start.is_none()
            && record.projected.is_some()
            && capture.capture.snapshot().state
                == sugarsubstitute_contracts::crash::CaptureState::Registered
        {
            record.attach = Some(
                capture
                    .attach(
                        sugarsubstitute_contracts::crash::CaptureTarget::Desktop(record.run),
                        resources.client,
                        resources.native,
                        resources.jobs,
                    )
                    .map_err(outer)?,
            );
        }
        Self::adopt_attachment(record, resources, admission_closed)?;
        let Some(start) = &mut record.start else {
            return Ok(());
        };
        let output_failure = start.output.as_mut().and_then(|output| {
            output
                .observe(
                    #[cfg(feature = "k1-native-qualification")]
                    record.parent.as_deref().and_then(ParentLifecycle::qualification_identity),
                )
                .err()
        });
        if record.parent.is_none()
            && let Some(completion) = start.profile.consume(batch)?
        {
            if let Some(cause) = completion.failure {
                return Err(Box::new(cause));
            }
            if completion.kind != sugarsubstitute_contracts::execution::TerminalKind::Success
                && !admission_closed
            {
                return Err("Actual profile acquisition stopped".into());
            }
        }
        if !admission_closed {
            start.acquire_profile(resources)?;
            start.reserve_channel(resources)?;
        }
        if !admission_closed && !incidents.seed_ready(record, capture, backend, resources)? {
            return Ok(());
        }
        let start = record.start.as_mut().ok_or("Actual desktop start disappeared")?;
        if !admission_closed && record.parent.is_none() {
            start.prepare_launch(executable, working, resources)?;
            if start.ticket.is_some() {
                Self::construct_parent(start, &mut record.parent, &mut record.preparation)?;
            }
        }
        if let Some(ticket) = start.ticket {
            let actual = resources
                .authority
                .registry()
                .ok_or("Actual process registry missing")?
                .snapshot(ticket.process())?;
            if actual.started && !record.replied && !admission_closed {
                let value = start.value()?;
                let reply_until = resources
                    .client
                    .now()?
                    .checked_add(std::time::Duration::from_secs(2))?
                    .min(value.until);
                resources.intake.server().control.reply(
                    value.entry_connection,
                    value.entry_request,
                    EntryOperationId::new(1)?,
                    reply_until,
                    EntryOutcome::Accepted(start.run),
                )?;
                record.replied = true;
            }
        }
        if record.parent.is_some() {
            let ticket = record
                .start
                .as_ref()
                .and_then(|start| start.ticket)
                .ok_or("Actual desktop process ticket missing")?;
            loop {
                let observation = resources
                    .authority
                    .registry_mut()
                    .ok_or("Actual process registry missing")?
                    .take_observation(ticket.process())?;
                let Some(observation) = observation else {
                    break;
                };
                with_parent(record, backend, Some(capture), resources, |parent, borrowed| {
                    Ok(parent.registry_observation(&observation, borrowed)?)
                })?;
            }
            let failed = record
                .parent
                .as_ref()
                .ok_or("Actual parent lifecycle missing")?
                .failed_incident(record.run, ticket.process())?;
            if let Some(fact) = failed {
                if record.failed.is_some_and(|original| original != fact) {
                    return Err("Original failed desktop fact changed".into());
                }
                record.failed = Some(fact);
            }
            let mut parent_activity =
                with_parent(record, backend, Some(capture), resources, |parent, borrowed| {
                    Ok(parent.activity(batch, borrowed)?)
                })
                .err();
            if parent_activity.as_ref().is_some_and(|cause| {
                cause.downcast_ref::<crate::production::parent_lifecycle::ParentError>()
                    == Some(&crate::production::parent_lifecycle::ParentError::DesktopStopped)
            }) && record.pending_desktop_failure.is_none()
                && record.failed_cause.is_none()
            {
                let original = match record.collection.original_budget() {
                    Some(original) => original,
                    None => sugarsubstitute_application::incident::IncidentOperationBudget::new(
                        resources.client.now()?,
                    )?,
                };
                let tighter = record
                    .parent
                    .as_ref()
                    .and_then(|parent| parent.shutdown())
                    .map(|budget| budget.settle_until);
                record.pending_desktop_failure =
                    Some(incident_reporting::PendingDesktopFailure::new(
                        parent_activity
                            .take()
                            .ok_or("Original desktop terminal failure missing")?,
                        original,
                        tighter,
                    ));
            }
            if parent_activity.is_none()
                && let Some(pending) = &record.pending_desktop_failure
            {
                let root_exited = resources
                    .authority
                    .registry()
                    .ok_or("Actual desktop registry missing")?
                    .snapshot(ticket.process())?
                    .root_exit
                    .is_some();
                match pending.disposition(
                    failed,
                    record.run,
                    ticket.process(),
                    root_exited,
                    resources.client.now()?,
                ) {
                    incident_reporting::DesktopFailureDisposition::Pending => {}
                    incident_reporting::DesktopFailureDisposition::Reportable => {
                        record.collection.retain_original_budget(pending.original())?;
                        record.failed_cause = Some(
                            record
                                .pending_desktop_failure
                                .take()
                                .ok_or("Original desktop terminal owner missing")?
                                .into_cause(),
                        );
                    }
                    incident_reporting::DesktopFailureDisposition::Fatal => {
                        return Err(record
                            .pending_desktop_failure
                            .take()
                            .ok_or("Original desktop terminal owner missing")?
                            .into_cause());
                    }
                }
            }
            let collection_progress = incident_collection::progress(
                record, incidents, capture, backend, resources, closing,
            )
            .err();
            if let Some(cause) = parent_activity {
                return Err(cause);
            }
            if let Some(cause) = collection_progress {
                return Err(cause);
            }
            let suspected = record.parent.as_ref().is_some_and(|parent| parent.ui_suspected());
            if suspected != record.ui_suspected {
                record.ui_suspected = suspected;
                tracing::warn!(operation="desktop_ui_watchdog",run=?record.run,suspected,
                    "Desktop watchdog status changed");
            }
        }
        if let Some(cause) = output_failure {
            return Err(cause);
        }
        if record.failed.is_some() && record.collection.source_retired() {
            if record.failed_shutdown.is_none() {
                record.failed_shutdown =
                    Some(match record.parent.as_ref().and_then(|parent| parent.shutdown()) {
                        Some(original) => original,
                        None => ShutdownBudget::new(resources.client.now()?)?,
                    });
            }
            let budget =
                record.failed_shutdown.ok_or("Original failed family cleanup budget missing")?;
            Self::close_record(record, backend, Some(capture), resources, budget)?;
            Self::adopt_failed_family(record, incidents)?;
        }
        Ok(())
    }
    /// Retain the sealed family witness while another actual policy effect occupies the Host.
    pub(super) fn adopt_failed_family(
        record: &mut Record,
        incidents: &mut incident_flow::IncidentFlow,
    ) -> Result<()> {
        if record.failed.is_none() || record.failed_family_adopted || incidents.host.closing() {
            return Ok(());
        }
        let Some(retired) = &record.retired else {
            return Ok(());
        };
        match incidents.host.observe_failed_family_retired(retired) {
            Ok(()) => record.failed_family_adopted = true,
            Err(crate::production::incident_host::HostIssue::Pending) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
