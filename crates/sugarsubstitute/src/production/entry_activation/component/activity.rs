// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance actual helper receipts and the same prepared desktop Job without a native surface.
use super::*;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryOperationId, EntryOutcome},
    execution::TerminalKind,
};
use sugarsubstitute_supervision::{
    ClaimLaunchJobs, LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
    profiles::PROFILE_OPERATION_BYTES,
};

impl ProfileComponent {
    pub(super) fn activity(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if let Some(job) = &mut self.projection
            && let Some(completion) = job.try_take_completion()
        {
            let identity = completion.scope_id == job.scope_id() && completion.spec == job.spec();
            self.projected = Some(completion);
            self.projection = None;
            if !identity {
                return Err("Original component projection completion identity changed".into());
            }
        }
        if let Some(completion) = &self.projected {
            let storage = completion.memory.claim((size_of::<Campaign>() + 1_048_576) as u64)?;
            let completion = self.projected.take().ok_or("Original component outcome absent")?;
            self.campaign = Some(Box::new(Campaign {
                projection: completion,
                custody: None,
                profile: None,
                watch: None,
                ticket: None,
                publication_issued: false,
                reply_until: None,
                replied: false,
                started: false,
                exited: false,
                registry_retired: false,
                released: false,
                _storage: storage,
            }));
        }
        if let Some(capture) = &mut self.capture {
            capture.observe(batch)?;
        }
        if self.complete {
            return Ok(());
        }
        let Some(campaign) = &mut self.campaign else { return Ok(()) };
        if let Some(profile) = &mut campaign.profile
            && let Some(done) = profile.consume(batch)?
        {
            if let Some(cause) = done.failure {
                return Err(Box::new(cause));
            }
            if done.kind != TerminalKind::Success && !self.closing {
                return Err("Original component profile job stopped".into());
            }
        }
        if self.closing {
            projection::return_unused_source(&mut campaign.projection.outcome);
            return Ok(());
        }
        if campaign.exited {
            self.retire_profile(resources)?;
            return Ok(());
        }
        let capture = self.capture.as_mut().ok_or("Actual component capture absent")?;
        let value = campaign.value()?;
        capture.start(&value.capture_root, value.until, resources)?;
        if !capture.registration_ready() {
            return Ok(());
        }
        let run = self.run.ok_or("Actual component run absent")?;
        if campaign.custody.is_none() {
            let value = campaign.value()?;
            let environment = value
                .desktop_environment
                .as_ref()
                .ok_or("Original component environment absent")?;
            campaign.custody = Some(
                CoordinatorCustody::new(
                    run.owner,
                    self.executable.clone(),
                    self.installation.clone(),
                    LaunchEnvironment::from_snapshot_utf16(environment.as_utf16(), &[])?,
                    value.until,
                    &mut super::super::progress::backend_resources(resources, Some(capture)),
                )
                .map_err(super::super::outer)?,
            );
        }
        let custody = campaign.custody.as_mut().ok_or("Actual component helper absent")?;
        if !custody
            .advance(&mut super::super::progress::backend_resources(resources, Some(capture)))
            .map_err(super::super::outer)?
        {
            return Ok(());
        }
        let parent = custody.established_parent().ok_or("Actual component establishment absent")?;
        let until = campaign.value()?.until;
        if campaign.profile.is_none() {
            let registry =
                resources.authority.registry().ok_or("Actual component registry absent")?;
            campaign.profile = Some(WindowsProfileLease::new(
                run,
                campaign.value()?.requested_profile.clone(),
                registry,
            )?);
            let watch = Self::spec(resources, JobClass::NativeObservation, 4096, None)?;
            let scope = ScopeSpec { id: resources.scopes.allocate()?, deadline: None };
            let spec = Self::spec(
                resources,
                JobClass::InfrastructureLifecycle,
                PROFILE_OPERATION_BYTES,
                Some(until),
            )?;
            campaign
                .profile
                .as_mut()
                .ok_or("Actual component profile absent")?
                .submit_custodied_acquire(
                    resources.client,
                    resources.authority.registry_mut().ok_or("Actual component registry absent")?,
                    resources.finite,
                    spec,
                    &parent,
                    (scope, watch),
                )?;
            campaign.watch = Some((scope, watch));
            return Ok(());
        }
        let profile = campaign.profile.as_mut().ok_or("Actual component profile absent")?;
        if !campaign.publication_issued && !profile.snapshot().pending {
            let spec = Self::spec(
                resources,
                JobClass::InfrastructureLifecycle,
                PROFILE_OPERATION_BYTES,
                Some(until),
            )?;
            campaign.publication_issued = profile
                .submit_custody_publication(resources.client, resources.finite, spec)?
                .is_some();
        }
        if !profile.snapshot().held
            || profile.snapshot().pending
            || !profile.custody_publication_ready()?
        {
            return Ok(());
        }
        if campaign.ticket.is_none() {
            let arguments =
                ["--internal-native-exit-fixture", "0"].map(|value| value.encode_utf16().collect());
            let command = WindowsLaunchCommand::from_utf16(
                &self.executable.as_os_str().encode_wide().collect::<Vec<_>>(),
                &self.installation.as_os_str().encode_wide().collect::<Vec<_>>(),
                &arguments,
            )?;
            let environment = campaign
                .value_mut()?
                .desktop_environment
                .take()
                .ok_or("Original component child environment transferred")?;
            let input = WindowsLaunchInput::new(command, environment)?;
            let profile = campaign.profile.as_mut().ok_or("Actual component profile absent")?;
            let prepared = profile.prepare_desktop_launch(input)?;
            let spec = Self::spec(
                resources,
                JobClass::InfrastructureLifecycle,
                prepared.required_reservation_bytes(),
                Some(until),
            )?;
            let (native_scope, root_watch) =
                campaign.watch.ok_or("Original component watch absent")?;
            campaign.ticket = Some(profile.submit_desktop_launch(
                resources.client,
                resources.authority.registry_mut().ok_or("Actual component registry absent")?,
                resources.finite,
                ClaimLaunchJobs { native_scope, creation: spec, root_watch },
                prepared,
            )?);
        }
        let ticket = campaign.ticket.ok_or("Actual component ticket absent")?;
        let actual = resources
            .authority
            .registry()
            .ok_or("Actual component registry absent")?
            .snapshot(ticket.process())?;
        if actual.failed {
            while let Some(observation) = resources
                .authority
                .registry_mut()
                .ok_or("Actual component registry absent")?
                .take_observation(ticket.process())?
            {
                if let sugarsubstitute_supervision::RegistryObservation::Failed { error, .. } =
                    observation
                {
                    return Err(Box::new(error));
                }
            }
            return Err("Original component registry failure remains retained".into());
        }
        if actual.started && !campaign.started {
            campaign.started = true;
            tracing::info!(operation="profile_component_started",run=?run,process=?ticket.process(),"Same prepared desktop Job resumed after actual helper Root ACK");
        }
        if actual.started && !campaign.replied {
            let reply_until = if let Some(original) = campaign.reply_until {
                original
            } else {
                let original = Self::reply_probe(resources.client.now()?, campaign.value()?.until)?;
                campaign.reply_until = Some(original);
                original
            };
            let value = campaign.value()?;
            resources.intake.server().control.reply(
                value.entry_connection,
                value.entry_request,
                EntryOperationId::new(1)?,
                reply_until,
                EntryOutcome::Accepted(run),
            )?;
            campaign.replied = true;
            projection::return_unused_source(&mut campaign.projection.outcome);
        }
        if let Some(code) = actual.root_exit {
            if code != 0 {
                return Err("Actual component child returned an unexpected exit".into());
            }
            if !campaign.exited {
                campaign.exited = true;
                self.shutdown = Some(ShutdownBudget::new(resources.client.now()?)?);
                tracing::info!(operation="profile_component_root_exited",run=?run,process=?ticket.process(),code,"Original contained component root exit observed");
            }
        }
        if campaign.exited {
            self.retire_profile(resources)?;
        }
        Ok(())
    }
}
