// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit the same genuine held carrier after durable ACK with complete rejection custody.
use super::*;
impl DesktopStart {
    /// Transfer actual durable ACK/current Root capture ownership and release the same held creation.
    /// Repeating that exact ACK after a successful begin is idempotent; changed ACK rejects.
    pub(in crate::production::entry_activation) fn commit_recovery(
        &mut self,
        previous: &WindowsProfileLease,
        ack: DurableAcknowledgement,
        root: CaptureRootSource,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        {
            let state = self.recovery.as_ref().ok_or("Actual recovery mode missing")?.state()?;
            if state.committed.is_some_and(|original| original != ack) {
                return Err("Recovery acknowledgement changed".into());
            }
            if state.creation_begun {
                return Ok(());
            }
            if state.cause.is_some() || state.closing.is_some() {
                return Err("Recovery startup already failed or closing".into());
            }
        }
        if self.recovery.as_ref().ok_or("Actual recovery mode missing")?.state()?.active.is_none() {
            if !self.recovery_acknowledgement_allowed(previous, resources)? {
                return Err("Actual recovery inputs are not ACK ready".into());
            }
            let state =
                self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
            let input = RootRecoverySessionInput {
                pipe: state.pipe.take().ok_or("Actual recovery control missing")?,
                grant: state.grant.clone(),
                identity: MainIdentity {
                    run: self.run,
                    launch: self.launch,
                    navigation: self.navigation,
                },
                carrier: self.ticket.ok_or("Actual recovery carrier missing")?,
                root,
                ack,
                desktop_watch: state
                    .desktop_watch
                    .take()
                    .ok_or("Actual recovery UI watch missing")?,
                handler_permit: state
                    .handler_permit
                    .take()
                    .ok_or("Actual recovery handler watch missing")?,
                enrollment: self.enrollment.take(),
                memory: state
                    .session_memory
                    .take()
                    .ok_or("Actual recovery metadata claim missing")?,
                outside: self.projection.memory.clone(),
            };
            state.committed = Some(ack);
            match RootRecoverySession::new(input) {
                Ok(session) => state.active = Some(session),
                Err(rejected) => {
                    let (cause, input) = rejected.into_parts();
                    state.pipe = Some(input.pipe);
                    state.desktop_watch = Some(input.desktop_watch);
                    state.handler_permit = Some(input.handler_permit);
                    self.enrollment = input.enrollment;
                    state.session_memory = Some(input.memory);
                    state._rejected_root = Some(input.root);
                    state.cause = Some(Box::new(cause));
                    return Err(
                        "Actual recovery session rejected; original owners and cause retained"
                            .into(),
                    );
                }
            }
        }
        let state = self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
        self.profile.begin_recovery_desktop_launch(
            resources.authority.registry_mut().ok_or("Actual recovery registry missing")?,
            &state.grant,
            previous,
            ack,
        )?;
        state.creation_begun = true;
        Ok(())
    }
}
