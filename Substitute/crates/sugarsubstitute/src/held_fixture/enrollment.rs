// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one immutable same-executable input at a time before actual child creation.
use super::*;
use std::{ffi::OsString, os::windows::io::AsRawHandle};
use sugarsubstitute_contracts::lifecycle::{
    InstanceId, ResourceGeneration, ResourceId, ResourceKey, RunGeneration, RunKey,
};
use sugarsubstitute_execution::windows_fixture::MIN_FIXTURE_SESSION_BYTES;
use sugarsubstitute_supervision::{
    FamilyBinding, LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
};

impl Fixture {
    /// Admit one session/creation decision; advance only after actual Ready/start.
    pub(super) fn enroll(&mut self) -> Result<bool> {
        if self.enrolling == HELD {
            return Ok(false);
        }
        let index = self.enrolling;
        if self.cases[index].session.is_none() {
            let mut spec = self.spec(JobClass::NativeObservation, MIN_FIXTURE_SESSION_BYTES)?;
            spec.deadline = Some(self.execution_deadline);
            let session =
                self.client.as_ref().ok_or("Campaign client missing")?.reserve_fixture_session(
                    self.event_scope.as_ref().ok_or("Campaign event scope missing")?,
                    spec,
                    self.cases[index].evidence.correlation,
                )?;
            self.cases[index].session = Some(session);
            return Ok(true);
        }
        if self.cases[index].evidence.prepared && self.cases[index].evidence.process.is_none() {
            self.launch_child(index)?;
            return Ok(true);
        }
        if self.cases[index].evidence.ready && self.cases[index].started {
            self.enrolling += 1;
            return Ok(true);
        }
        Ok(false)
    }
    /// Move the four-role launch pin into the registry's durable native ownership.
    fn launch_child(&mut self, index: usize) -> Result<()> {
        let case = &self.cases[index];
        let events = case
            .session
            .as_ref()
            .ok_or("Campaign session missing")?
            .control
            .take_child_events()?
            .ok_or("Prepared session lacks launch artifact")?;
        let handles = events.borrow_inheritance();
        let mut arguments = vec![OsString::from("--internal-native-held-child")];
        for (flag, value) in [
            ("--ready", handles.ready.as_raw_handle() as usize as u64),
            ("--release", handles.release.as_raw_handle() as usize as u64),
            ("--challenge", handles.challenge.as_raw_handle() as usize as u64),
            ("--response", handles.response.as_raw_handle() as usize as u64),
            ("--deadline-qpc", self.child_deadline.ticks()),
            ("--qpc-frequency", self.child_deadline.frequency()),
            ("--correlation", case.evidence.correlation.get()),
            ("--exit-code", u64::from(case.evidence.expected)),
        ] {
            arguments.push(flag.into());
            arguments.push(value.to_string().into());
        }
        let input = self.input(&arguments, Some(events))?;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let native = self.native_scope_spec()?;
        let binding = FamilyBinding::Desktop(RunKey {
            owner: self.owner_key,
            instance: InstanceId::new(u64::try_from(index)? + 1)?,
            generation: RunGeneration::new(1)?,
        });
        let ticket =
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_launch(
                self.client.as_ref().ok_or("Campaign client missing")?,
                self.creation_scope.as_ref().ok_or("Campaign creation scope missing")?,
                native,
                creation,
                watch,
                binding,
                input,
            )?;
        self.cases[index].creation_job = Some(creation.id);
        self.cases[index].evidence.bind(ticket.process(), watch.id)
    }
    /// Enroll a later watch only after all64 older live watches have actual bindings.
    /// Stable native FIFO and absence of early retirement make its exit proof causal.
    pub(super) fn launch_sentinel(&mut self) -> Result<()> {
        let input = self.input(&["--internal-native-exit-fixture".into(), "259".into()], None)?;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        if self.cases.iter().any(|case| {
            !case.started
                || !case.evidence.ready
                || case.evidence.cancel_requested.is_some()
                || case.evidence.retirement.is_some()
                || case.evidence.exited.is_some()
                || case.evidence.root_job.is_none_or(|id| id >= watch.id)
        }) {
            return Err("Sentinel requires64 earlier live bound root watches".into());
        }
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let native = self.native_scope_spec()?;
        let binding = FamilyBinding::Managed(ResourceKey {
            owner: self.owner_key,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        });
        let ticket =
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_launch(
                self.client.as_ref().ok_or("Campaign client missing")?,
                self.creation_scope.as_ref().ok_or("Campaign creation scope missing")?,
                native,
                creation,
                watch,
                binding,
                input,
            )?;
        self.sentinel.bind(ticket.process())?;
        self.sentinel_watch = Some(watch.id);
        self.sentinel_creation = Some(creation.id);
        Ok(())
    }
    /// Construct only self-executable inputs with an explicit empty environment.
    fn input(
        &self,
        arguments: &[OsString],
        events: Option<sugarsubstitute_execution::windows_fixture::FixtureChildEvents>,
    ) -> Result<WindowsLaunchInput> {
        let executable = std::env::current_exe()?;
        let cwd = executable.parent().ok_or("Campaign EXE has no parent")?;
        let command = WindowsLaunchCommand::from_os(&executable, cwd, arguments)?;
        let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
        Ok(match events {
            Some(events) => WindowsLaunchInput::with_fixture_events(command, environment, events)?,
            None => WindowsLaunchInput::new(command, environment)?,
        })
    }
}
