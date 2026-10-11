// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain four-event bootstrap artifacts and exact claim-bound launch inputs.
use super::*;
use std::{ffi::OsString, os::windows::io::AsRawHandle};
impl Fixture {
    pub(super) fn start_child(
        &mut self,
        role: Role,
        installation: &str,
        abortive: bool,
    ) -> Result<bool> {
        if self.child.is_none() {
            let index = self.results.len();
            if index >= 7 {
                return Err("Claim campaign child inventory exhausted".into());
            }
            let spec = self.spec(JobClass::NativeObservation, MIN_FIXTURE_SESSION_BYTES, true)?;
            let session = self
                .client
                .as_ref()
                .ok_or("Claim campaign client missing")?
                .reserve_fixture_session(
                    self.native_scope.as_ref().ok_or("Claim campaign native scope missing")?,
                    spec,
                    NativeCorrelation::new(index as u64 + 1)?,
                )?;
            self.child = Some(ChildCase {
                role,
                index,
                session: Some(session),
                family: None,
                prepared: false,
                ready: false,
                challenge_sent: false,
                responded: false,
                release_sent: false,
                abortive,
                event_cancel_requested: false,
                event_notice: None,
                event_retired: false,
                event_report: None,
                receipt: None,
                read: None,
                read_notice: false,
            });
            return Ok(true);
        }
        let case = self.child.as_ref().ok_or("Claim campaign child missing")?;
        if case.role != role || case.abortive != abortive {
            return Err("Claim campaign attempted a different live child role".into());
        }
        if !case.prepared || case.family.is_some() {
            return Ok(false);
        }
        let key = self.key.ok_or("Claim campaign native owner missing")?;
        let events = case
            .session
            .as_ref()
            .ok_or("Claim campaign session missing")?
            .control
            .take_child_events()?
            .ok_or("Prepared claim session lacks launch artifact")?;
        let handles = events.borrow_inheritance();
        let mut arguments = vec![OsString::from("--internal-native-claim-child")];
        for (flag, value) in [
            ("--role", role.wire().into()),
            ("--installation", self.options.workspace.join(installation).into_os_string()),
            ("--backend", self.options.workspace.join("backend").into_os_string()),
            (
                "--receipt",
                self.options.workspace.join(format!("child-{}.json", case.index)).into_os_string(),
            ),
            ("--namespace", hex_text(key.namespace.bytes()).into()),
            ("--epoch", hex_text(key.epoch.bytes()).into()),
        ] {
            arguments.push(flag.into());
            arguments.push(value);
        }
        for (flag, value) in [
            ("--ready", handles.ready.as_raw_handle() as usize as u64),
            ("--release", handles.release.as_raw_handle() as usize as u64),
            ("--challenge", handles.challenge.as_raw_handle() as usize as u64),
            ("--response", handles.response.as_raw_handle() as usize as u64),
            ("--deadline-qpc", self.child_deadline.ticks()),
            ("--qpc-frequency", self.child_deadline.frequency()),
            ("--correlation", case.index as u64 + 1),
        ] {
            arguments.push(flag.into());
            arguments.push(value.to_string().into());
        }
        let command = WindowsLaunchCommand::from_os(
            &std::env::current_exe()?,
            &self.options.workspace,
            &arguments,
        )?;
        let input = WindowsLaunchInput::with_fixture_events(
            command,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            events,
        )?;
        let (watch, creation) = super::super::specification::launch(
            &mut self.next_job,
            input.required_reservation_bytes(),
            self.execution_deadline,
        )?;
        let native = self.family_scope()?;
        let binding = FamilyBinding::Desktop(RunKey {
            owner: key,
            instance: InstanceId::new(self.results.len() as u64 + 1)?,
            generation: RunGeneration::new(1)?,
        });
        let ticket =
            self.registry.as_mut().ok_or("Claim campaign registry missing")?.submit_launch(
                self.client.as_ref().ok_or("Claim campaign client missing")?,
                self.scope.as_ref().ok_or("Claim campaign work scope missing")?,
                native,
                creation,
                watch,
                binding,
                input,
            )?;
        self.child.as_mut().ok_or("Claim campaign child missing")?.family =
            Some(Family::new(ticket, if abortive { 1 } else { 37 }, creation.id, watch.id)?);
        Ok(true)
    }
    pub(super) fn launch_managed(&mut self) -> Result<bool> {
        if self.managed.is_some() {
            return Ok(false);
        }
        let input = self.claims.prepare_managed_launch(
            self.resource.ok_or("Claim campaign resource missing")?,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
        )?;
        let (root_watch, creation) = super::super::specification::launch(
            &mut self.next_job,
            input.required_reservation_bytes(),
            self.execution_deadline,
        )?;
        let native_scope = self.family_scope()?;
        let ticket = self.claims.submit_managed_launch(
            self.client.as_ref().ok_or("Claim campaign client missing")?,
            self.registry.as_mut().ok_or("Claim campaign registry missing")?,
            self.scope.as_ref().ok_or("Claim campaign scope missing")?,
            ClaimLaunchJobs { native_scope, creation, root_watch },
            input,
        )?;
        self.managed = Some(Family::new(ticket, 37, creation.id, root_watch.id)?);
        Ok(true)
    }
}
