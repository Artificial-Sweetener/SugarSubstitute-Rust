// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt each retained desktop's actual attachment, profile and lifecycle owners.
use super::progress::with_parent;
use super::*;
impl ProductionActivation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn progress_record(
        record: &mut Record,
        capture: &mut CaptureRegistration,
        backend: &mut CoordinatorBackend,
        executable: &std::path::Path,
        working: &std::path::Path,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
        closing: bool,
    ) -> Result<()> {
        if let Some(completion) = &record.projected {
            match &completion.outcome {
                JobOutcome::Success(value) if !closing => {
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
        if !closing
            && record.attach.is_none()
            && record.start.is_none()
            && record.projected.is_some()
            && capture.capture.snapshot().state
                == sugarsubstitute_contracts::crash::CaptureState::Registered
        {
            record.attach = Some(capture.attach(record.run, resources)?);
        }
        Self::adopt_attachment(record, resources, closing)?;
        let Some(start) = &mut record.start else { return Ok(()) };
        if record.parent.is_none()
            && let Some(completion) = start.profile.consume(batch)?
        {
            if let Some(cause) = completion.failure {
                return Err(Box::new(cause));
            }
            if completion.kind != sugarsubstitute_contracts::execution::TerminalKind::Success
                && !closing
            {
                return Err("Actual profile acquisition stopped".into());
            }
        }
        if !closing {
            start.acquire_profile(resources)?;
            start.reserve_channel(resources)?;
            if record.parent.is_none() {
                start.prepare_launch(executable, working, resources)?;
                if start.ticket.is_some() {
                    Self::construct_parent(start, &mut record.parent, &mut record.preparation)?;
                }
            }
        }
        if let Some(ticket) = start.ticket {
            let actual = resources
                .authority
                .registry()
                .ok_or("Actual process registry missing")?
                .snapshot(ticket.process())?;
            if actual.started && !record.replied && !closing {
                let value = start.value()?;
                resources.intake.server().control.reply(
                    value.entry_connection,
                    value.entry_request,
                    EntryOperationId::new(1)?,
                    value.until,
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
                let Some(observation) = observation else { break };
                with_parent(record, backend, resources, |parent, borrowed| {
                    Ok(parent.registry_observation(&observation, borrowed)?)
                })?;
            }
            with_parent(record, backend, resources, |parent, borrowed| {
                Ok(parent.activity(batch, borrowed)?)
            })?;
            let suspected = record.parent.as_ref().is_some_and(|parent| parent.ui_suspected());
            if suspected != record.ui_suspected {
                record.ui_suspected = suspected;
                tracing::warn!(operation="desktop_ui_watchdog",run=?record.run,suspected,
                    "Desktop watchdog status changed");
            }
        }
        Ok(())
    }
}
