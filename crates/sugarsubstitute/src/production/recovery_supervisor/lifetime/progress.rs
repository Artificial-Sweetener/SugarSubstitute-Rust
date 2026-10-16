// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance only from genuine startup, registration, native write and child observations.
use super::*;
use sugarsubstitute_contracts::recovery_supervisor::wire::{
    RecoveryCommand, RecoveryReceiver, decode,
};
impl Lifetime {
    pub(super) fn progress(&mut self) -> Result<HeadlessStep> {
        let client = self.client.as_ref().ok_or("Recovery execution absent")?;
        let finite = self.finite.as_ref().ok_or("Recovery finite scope absent")?;
        let native = self.native.as_ref().ok_or("Recovery native scope absent")?;
        let channel = self.channel.as_mut().ok_or("Recovery channel absent")?;
        if !matches!(self.stage, Stage::Running | Stage::ExitedQueued | Stage::Finished)
            && client.now()? >= channel.original_until()
        {
            return Err("Original recovery setup deadline expired".into());
        }
        if let Some(sent) = channel.take_sent() {
            self.stage = match (sent, self.stage) {
                (Sent::Hello, stage) => stage,
                (Sent::StartupAccepted, Stage::AcceptedQueued) => {
                    let capture = self.capture.as_mut().ok_or("Fresh capture owner absent")?;
                    capture.start(client, native, finite, &self.jobs)?;
                    Stage::Registering
                }
                (Sent::CaptureRegistered, Stage::RegisteredQueued) => {
                    self.capture
                        .as_mut()
                        .ok_or("Fresh capture owner absent")?
                        .attach(client, native, &self.jobs)?;
                    Stage::Attaching
                }
                (Sent::DesktopCreated, Stage::CreatedQueued) => Stage::Enrollment,
                (Sent::DesktopExited, Stage::ExitedQueued) => Stage::Finished,
                _ => return Err("Recovery actual write phase rejected".into()),
            };
        }
        if self.origin.is_some()
            && let Some(frame) = channel.control_frame()
        {
            match decode(frame.as_bytes(), RecoveryReceiver::Supervisor)?.command {
                RecoveryCommand::Stop(counter) => {
                    channel.control().validate_current_admission()?;
                    let until = client.decode_lifecycle_deadline(sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(counter.ticks(), counter.frequency())?)?;
                    self.cleanup.root_stop(client.now()?, until)?;
                    return Ok(HeadlessStep::Shutdown);
                }
                RecoveryCommand::Failed(code) => {
                    return Err(format!("Actual Root stopped recovery: {code:?}").into());
                }
                RecoveryCommand::DesktopEnrolled(_) => {}
                _ => return Err("Recovery post-startup Root command rejected".into()),
            }
        }
        match self.stage {
            Stage::Startup => {
                if self.origin.is_none()
                    && let Some(startup) = channel.take_startup()
                {
                    if channel.key() != Some(startup.origin.key()) {
                        return Err(
                            "Actual recovery startup source belongs to another Root session".into(),
                        );
                    }
                    self.capture = Some(Capture::new(startup.origin.clone()));
                    self.origin = Some(startup.origin);
                    self.startup = Some(startup.startup);
                }
                if self.origin.is_some()
                    && channel.try_send(&Outgoing::StartupAccepted, channel.original_until())?
                {
                    self.stage = Stage::AcceptedQueued;
                }
            }
            Stage::Registering => {
                let capture = self.capture.as_ref().ok_or("Fresh capture owner absent")?;
                if capture.ready() {
                    let (supervisor, handler) = capture.identities()?;
                    if channel.try_send(
                        &Outgoing::CaptureRegistered { supervisor, handler },
                        channel.original_until(),
                    )? {
                        self.stage = Stage::RegisteredQueued;
                    }
                }
            }
            Stage::Attaching => {
                if let Some(pin) =
                    self.capture.as_mut().ok_or("Fresh capture owner absent")?.take_pin()
                {
                    let startup =
                        self.startup.take().ok_or("Original delegated Desktop startup absent")?;
                    let job = self.role.job.take().ok_or("Original inherited Job absent")?;
                    let origin =
                        self.origin.as_ref().ok_or("Original recovery source absent")?.clone();
                    self.child_input = match RecoveryChildInput::new(
                        job,
                        CapturedLifecycleStart::new(pin, startup),
                        move |section, context| {
                            super::super::command::build(&origin, section, context)
                        },
                    ) {
                        Ok(input) => Some(input),
                        Err(rejected) => {
                            self.role.job = Some(rejected.job);
                            drop(rejected.captured);
                            return Err(Box::new(rejected.cause));
                        }
                    };
                    let watch = JobSpec {
                        id: self.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::NativeObservation,
                        deadline: None,
                        reservation_bytes: 4096,
                        replacement: None,
                    };
                    let spec = JobSpec {
                        id: self.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(channel.original_until()),
                        reservation_bytes: self
                            .child_input
                            .as_ref()
                            .ok_or("Original recovery child input absent")?
                            .required_reservation_bytes(),
                        replacement: None,
                    };
                    match RecoveryChildCreation::submit(
                        client,
                        finite,
                        spec,
                        native,
                        watch,
                        self.origin.as_ref().ok_or("Original recovery source absent")?,
                        &mut self.child_input,
                    ) {
                        Ok(child) => self.child = Some(child),
                        Err(rejected) => {
                            self.child = Some(rejected.owner);
                            return Err(Box::new(rejected.cause));
                        }
                    }
                    self.stage = Stage::Creating;
                }
            }
            Stage::Creating => {
                if let Some(process) = self.child.as_ref().and_then(RecoveryChildCreation::created)
                    && channel
                        .try_send(&Outgoing::DesktopCreated(process), channel.original_until())?
                {
                    self.stage = Stage::CreatedQueued;
                }
            }
            Stage::Enrollment => {
                let child = self.child.as_ref().ok_or("Actual suspended Desktop owner absent")?;
                let expected = child.created().ok_or("Actual suspended Desktop identity absent")?;
                self.enrollment = channel.take_enrollment(
                    self.origin.as_ref().ok_or("Original recovery source absent")?,
                    expected,
                )?;
                if self.enrollment.is_some() {
                    child.enroll(&mut self.enrollment)?;
                    self.stage = Stage::Resuming;
                }
            }
            Stage::Resuming if self.child_status.is_some_and(|status| status.resumed) => {
                self.setup_wake = None;
                self.stage = Stage::Running;
            }
            Stage::Running => {
                if let Some(status) = self.child_status
                    && let (Some(process), Some(status)) = (status.created, status.exit)
                {
                    self.cleanup.capture(client.now()?)?;
                    if channel.try_send(
                        &Outgoing::DesktopExited { process, status },
                        self.cleanup.until().ok_or("Original capture cutoff absent")?,
                    )? {
                        self.stage = Stage::ExitedQueued;
                    }
                }
            }
            Stage::Finished => return Ok(HeadlessStep::Shutdown),
            _ => {}
        }
        Ok(HeadlessStep::Active)
    }
}
