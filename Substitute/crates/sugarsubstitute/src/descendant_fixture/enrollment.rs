// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create one owned outer Job before the forwarding root can execute its fixed role.
use super::*;
use std::{ffi::OsString, os::windows::io::AsRawHandle};
use sugarsubstitute_supervision::{LaunchEnvironment, WindowsLaunchCommand};
impl Fixture {
    pub(super) fn enroll_events(&mut self) -> Result<()> {
        let mut spec = self.spec(JobClass::NativeObservation, MIN_FIXTURE_SESSION_BYTES)?;
        spec.deadline = Some(self.deadline);
        self.session =
            Some(self.client.as_ref().ok_or("Descendant client")?.reserve_fixture_session(
                self.event_scope.as_ref().ok_or("Descendant event scope")?,
                spec,
                NativeCorrelation::new(1)?,
            )?);
        Ok(())
    }
    pub(super) fn enroll_root(&mut self) -> Result<()> {
        if self.key.is_some() || self.closing || self.evidence.retired {
            return Ok(());
        }
        let Some(events) =
            self.session.as_ref().ok_or("Descendant session")?.control.take_child_events()?
        else {
            return Ok(());
        };
        let module = std::env::current_exe()?;
        let cwd = module.parent().ok_or("Descendant executable parent")?;
        let mut args = vec![OsString::from("--internal-native-forwarding-root")];
        {
            let view = events.borrow_inheritance();
            for (flag, value) in [
                ("--ready", view.ready.as_raw_handle() as usize as u64),
                ("--release", view.release.as_raw_handle() as usize as u64),
                ("--challenge", view.challenge.as_raw_handle() as usize as u64),
                ("--response", view.response.as_raw_handle() as usize as u64),
                ("--deadline-qpc", self.qpc_deadline.ticks()),
                ("--qpc-frequency", self.qpc_deadline.frequency()),
                ("--correlation", 1),
                ("--exit-code", 0),
            ] {
                args.push(flag.into());
                args.push(value.to_string().into());
            }
        }
        let input = WindowsLaunchInput::with_fixture_events(
            WindowsLaunchCommand::from_os(&module, cwd, &args)?,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            events,
        )?;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let ticket =
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_launch(
                self.client.as_ref().ok_or("Descendant client")?,
                self.creation_scope.as_ref().ok_or("Descendant creation scope")?,
                ScopeSpec { id: ScopeId::new(3)?, deadline: None },
                creation,
                watch,
                FamilyBinding::Desktop(RunKey {
                    owner: self.owner_key,
                    instance: InstanceId::new(1)?,
                    generation: RunGeneration::new(1)?,
                }),
                input,
            )?;
        self.key = Some(ticket.process());
        self.evidence.root_job = Some(watch.id);
        Ok(())
    }
    pub(super) fn cleanup(&mut self) -> Result<()> {
        if !self.cleanup_needed || self.cleanup_job.is_some() {
            return Ok(());
        }
        let Some(key) = self.key else { return Ok(()) };
        let snapshot =
            self.registry.as_ref().ok_or("Actual registry is not prepared")?.snapshot(key)?;
        // A live member watch does not block finite sealing/force cleanup. This
        // consumer's cleanup_job tracks the actual finite operation separately.
        if self.stop.is_none() || snapshot.creation_pending || snapshot.settled {
            return Ok(());
        }
        let watch = if snapshot.member_watch_required {
            Some(self.spec(JobClass::NativeObservation, 4096)?)
        } else {
            None
        };
        let spec = self.spec(JobClass::InfrastructureLifecycle, 4096)?;
        self.cleanup_needed = false;
        self.cleanup_job = Some(
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_cleanup_step(
                self.client.as_ref().ok_or("Descendant client")?,
                key,
                self.creation_scope.as_ref().ok_or("Descendant scope")?,
                spec,
                watch,
            )?,
        );
        if let Some(watch) = watch {
            self.member_job = Some(watch.id);
            self.evidence.bind_member(watch.id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_descendant_family_does_not_attempt_another_launch() -> Result<()> {
        let mut fixture = Fixture::new()?;
        fixture.evidence.retired = true;
        fixture.enroll_root()?;
        assert!(fixture.key.is_none());
        assert!(fixture.session.is_none());
        Ok(())
    }
}
