// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance only fully completed native writes, retaining one pending admission.
use super::*;
use std::os::windows::ffi::OsStrExt;
/// Passive receive phase only: authenticated observation never substitutes native watch retirement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum RetirementReceipt {
    #[default]
    Awaiting,
    Received,
}
impl RetirementReceipt {
    /// Called after the existing same-peer/key/sequence decoder and close-order checks.
    pub(super) fn receive(&mut self, closing: bool) -> Result<(), SessionCode> {
        if !closing || *self == Self::Received {
            return Err(SessionCode::Order);
        }
        *self = Self::Received;
        Ok(())
    }
    /// A final accepted message ends receive admission while independent owners keep retiring.
    pub(super) const fn read_open(self) -> bool {
        matches!(self, Self::Awaiting)
    }
    /// Report final protocol receipt independently of physical worker retirement.
    pub(super) const fn received(self) -> bool {
        matches!(self, Self::Received)
    }
    /// Invoke the existing native read admission only while the final message remains unreceived.
    pub(super) fn admit_read<E>(self, effect: impl FnOnce() -> Result<(), E>) -> Result<(), E> {
        if self.read_open() { effect() } else { Ok(()) }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Stage {
    #[default]
    Identity,
    Startup(usize),
    Root(usize),
    Commit,
    Active,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Identity,
    Startup { next: usize, total: usize },
    Root { next: usize, total: usize },
    Commit,
    Enrolled,
    Stop,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Pending {
    operation: LifecyclePipeOperationId,
    sequence: u64,
    bytes: usize,
    role: Role,
    issued: bool,
}
#[derive(Default)]
pub(super) struct SendPlan {
    pub(super) stage: Stage,
    sequence: u64,
    pending: Option<Pending>,
}
impl SendPlan {
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn next_sequence(&self) -> Result<u64, SessionCause> {
        self.sequence.checked_add(1).ok_or(SessionCause::Protocol(SessionCode::Exhausted))
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn install(
        &mut self,
        operation: LifecyclePipeOperationId,
        bytes: usize,
        role: Role,
    ) -> Result<(), SessionCause> {
        if self.pending.is_some() || bytes == 0 || bytes > RECOVERY_WIRE_BYTES {
            return Err(SessionCause::Protocol(SessionCode::Order));
        }
        self.pending = Some(Pending {
            operation,
            sequence: self.next_sequence()?,
            bytes,
            role,
            issued: false,
        });
        Ok(())
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn issued(
        &mut self,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), SessionCause> {
        let pending = self.pending.as_mut().ok_or(SessionCause::Protocol(SessionCode::Order))?;
        if pending.operation != operation || pending.issued {
            return Err(SessionCause::Protocol(SessionCode::Binding));
        }
        pending.issued = true;
        Ok(())
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn complete(
        &mut self,
        operation: LifecyclePipeOperationId,
        bytes: u32,
    ) -> Result<Role, SessionCause> {
        let pending = self.pending.ok_or(SessionCause::Protocol(SessionCode::Order))?;
        if pending.operation != operation || !pending.issued || pending.bytes != bytes as usize {
            return Err(SessionCause::Protocol(SessionCode::Binding));
        }
        let stage = match (self.stage, pending.role) {
            (Stage::Identity, Role::Identity) => Stage::Startup(0),
            (Stage::Startup(offset), Role::Startup { next, total })
                if next > offset && next <= total =>
            {
                if next == total {
                    Stage::Root(0)
                } else {
                    Stage::Startup(next)
                }
            }
            (Stage::Root(offset), Role::Root { next, total }) if next > offset && next <= total => {
                if next == total {
                    Stage::Commit
                } else {
                    Stage::Root(next)
                }
            }
            (Stage::Commit, Role::Commit) => Stage::Active,
            (Stage::Active, Role::Enrolled) => Stage::Active,
            (_, Role::Stop) => self.stage,
            _ => return Err(SessionCause::Protocol(SessionCode::Order)),
        };
        self.stage = stage;
        self.sequence = pending.sequence;
        self.pending = None;
        Ok(pending.role)
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn revoked(
        &mut self,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), SessionCause> {
        if self.pending.is_none_or(|pending| pending.operation != operation || pending.issued) {
            return Err(SessionCause::Protocol(SessionCode::Binding));
        }
        self.pending = None;
        Ok(())
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }
}
impl State {
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn send_next(&mut self, root_path: &Path) -> Result<(), SessionCause> {
        if self.peer.is_none() || self.plan.pending() || self.pipe_terminal.is_some() {
            return Ok(());
        }
        let role = if let Some(close) = &self.closing {
            if close.sent
                || close.control_closed
                || self.failures.first.is_some()
                || self.plan.stage != Stage::Active
            {
                return Ok(());
            }
            Role::Stop
        } else {
            if self.failures.first.is_some() {
                return Ok(());
            }
            match self.plan.stage {
                Stage::Identity => Role::Identity,
                Stage::Startup(_) => Role::Startup { next: 0, total: self.grant.startup_bytes() },
                Stage::Root(_) => {
                    Role::Root { next: 0, total: root_path.as_os_str().encode_wide().count() }
                }
                Stage::Commit => Role::Commit,
                Stage::Active => {
                    if self.enrolled_reply {
                        return Ok(());
                    }
                    let work = match self.work.try_lock() {
                        Ok(work) => work,
                        Err(TryLockError::WouldBlock) => return Ok(()),
                        Err(TryLockError::Poisoned(_)) => {
                            return Err(SessionCause::Protocol(SessionCode::Worker));
                        }
                    };
                    if !work
                        .desktop
                        .as_ref()
                        .is_some_and(RecoveryEnrolledDesktop::lifecycle_enrolled)
                        || work.handler.as_ref().is_none_or(|handler| {
                            handler.terminal().is_some() || handler.failure().is_some()
                        })
                    {
                        return Ok(());
                    }
                    Role::Enrolled
                }
            }
        };
        let Some(mut buffer) =
            self.pipe.control().take_write_buffer().map_err(SessionCause::Pipe)?
        else {
            return Ok(());
        };
        let sequence = self.plan.next_sequence()?;
        let (bytes, role) = match role {
            Role::Identity => (
                self.pipe
                    .control()
                    .encode_original_desktop_identity(
                        &self.grant,
                        sequence,
                        self.identity,
                        &mut buffer,
                    )
                    .map_err(SessionCause::Startup)?,
                role,
            ),
            Role::Startup { total, .. } => {
                let Stage::Startup(offset) = self.plan.stage else {
                    return Err(SessionCause::Protocol(SessionCode::Order));
                };
                let (bytes, next) = self
                    .pipe
                    .control()
                    .encode_original_startup(&self.grant, sequence, offset, &mut buffer)
                    .map_err(SessionCause::Startup)?;
                (bytes, Role::Startup { next, total })
            }
            Role::Root { total, .. } => {
                let Stage::Root(offset) = self.plan.stage else {
                    return Err(SessionCause::Protocol(SessionCode::Order));
                };
                let identity = self.root_identity;
                let (bytes, next) = self
                    .pipe
                    .control()
                    .encode_original_capture_root_path(
                        &self.grant,
                        sequence,
                        identity,
                        root_path,
                        offset,
                        &mut buffer,
                    )
                    .map_err(SessionCause::Startup)?;
                (bytes, Role::Root { next, total })
            }
            Role::Commit => (
                self.pipe
                    .control()
                    .encode_original_commit(&self.grant, sequence, self.ack, &mut buffer)
                    .map_err(SessionCause::Startup)?,
                role,
            ),
            Role::Enrolled => {
                let work = match self.work.try_lock() {
                    Ok(work) => work,
                    Err(TryLockError::WouldBlock) => return Ok(()),
                    Err(TryLockError::Poisoned(_)) => {
                        return Err(SessionCause::Protocol(SessionCode::Worker));
                    }
                };
                let desktop = work
                    .desktop
                    .as_ref()
                    .filter(|desktop| desktop.lifecycle_enrolled())
                    .ok_or(SessionCause::Protocol(SessionCode::Binding))?;
                if work.handler.as_ref().is_none_or(|handler| {
                    handler.terminal().is_some() || handler.failure().is_some()
                }) {
                    return Ok(());
                }
                self.grant.check().map_err(SessionCause::Admission)?;
                (
                    encode(
                        &RecoveryFrame {
                            key: self.grant.key(),
                            sequence,
                            command: RecoveryCommand::DesktopEnrolled(desktop.actual()),
                        },
                        RecoveryReceiver::Supervisor,
                        buffer.as_mut_bytes(),
                    )
                    .map_err(SessionCause::Wire)?,
                    role,
                )
            }
            Role::Stop => {
                let close =
                    self.closing.as_ref().ok_or(SessionCause::Protocol(SessionCode::Order))?;
                (
                    encode(
                        &RecoveryFrame {
                            key: self.grant.key(),
                            sequence,
                            command: RecoveryCommand::Stop(close.counter),
                        },
                        RecoveryReceiver::Supervisor,
                        buffer.as_mut_bytes(),
                    )
                    .map_err(SessionCause::Wire)?,
                    role,
                )
            }
        };
        let deadline =
            self.closing.as_ref().map_or(self.grant.original_budget().until(), |close| close.until);
        let operation = self.next_operation()?;
        // Install durable matching metadata before native admission; rejected writes never advance it.
        self.plan.install(operation, bytes, role)?;
        match self.pipe.control().write(operation, deadline, buffer, bytes) {
            Ok(admission) => {
                self.admission = Some(admission);
                Ok(())
            }
            Err(rejected) => Err(SessionCause::Pipe(rejected.error)),
        }
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn read_next(&mut self) -> Result<(), SessionCause> {
        if self.read.is_some()
            || self.pipe_terminal.is_some()
            || self.closing.as_ref().is_some_and(|close| close.control_closed)
            || !self.connected
        {
            return Ok(());
        }
        if self.failures.first.is_some() && self.closing.is_none() {
            return Ok(());
        }
        // Do not leave an original30s receive pending after the last startup reply.
        // DesktopCreated pauses reads until actual Root enrollment and its complete reply.
        if self.desktop_advisory.is_some() && !self.enrolled_reply && self.closing.is_none() {
            return Ok(());
        }
        self.subordinate_retired.admit_read(|| {
            let operation = self.next_operation()?;
            if self.peer.is_some() && (self.enrolled_reply || self.closing.is_some()) {
                self.pipe.control().read_passive(operation).map_err(SessionCause::Pipe)?;
            } else if self.closing.is_none() && self.failures.first.is_none() {
                self.pipe
                    .control()
                    .read(operation, self.grant.original_budget().until())
                    .map_err(SessionCause::Pipe)?;
            } else {
                return Ok(());
            }
            self.read = Some(operation);
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "framing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "final_receive_tests.rs"]
mod final_receive_tests;
