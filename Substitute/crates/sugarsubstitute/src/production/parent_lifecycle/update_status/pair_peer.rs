// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The coordinator's real per-desktop lifecycle actor, fed by frames in the two-peer harness.
//!
//! [`ParentLifecycle`] keeps all of its own state here: its update snapshot, the child's
//! restart intent and check requests, its close request and its desktop exit transaction. Its
//! native pipe owners need an authenticated peer process, so this peer routes the decoded
//! frames itself, exactly as `receive.rs` and `requests.rs` route RestartToApply,
//! CheckForUpdates, ExitIntent, ExitCompleted and MainRevealed, and writes exactly when
//! `send_update_status` and `send_child_close` would: replies first, then an owed close
//! request, then the newest update snapshot, each completing as soon as the window reads it.
use super::super::{ParentLifecycle, child_close::ChildClose};
use crate::lifecycle_harness::{HarnessResult, WireEnd};
use std::collections::VecDeque;
use sugarsubstitute_application::supervision::DesktopExitTransaction;
use sugarsubstitute_contracts::{
    execution::Moment,
    lifecycle::{ConnectionId, DesktopObservation, ProcessKey, RunKey, ShutdownBudget},
    startup::LaunchId,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireHeader, WireMessage, WirePayload,
};

/// One registered desktop's real lifecycle actor and the replies it still owes.
pub(in crate::production) struct DesktopPeer {
    parent: ParentLifecycle,
    process: ProcessKey,
    replies: VecDeque<WireHeader>,
    revealed: bool,
    completed: bool,
}

impl DesktopPeer {
    /// A registered desktop of `run`, created as registry entry `process`.
    /// # Errors
    /// Returns the actor's storage or identity failure.
    pub(in crate::production) fn new(
        run: RunKey,
        launch: LaunchId,
        process: ProcessKey,
    ) -> HarnessResult<Self> {
        let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
            ParentLifecycle::storage_bytes(),
        )
        .map_err(|cause| cause.to_string())?;
        let mut causes = Some(
            super::super::causes::BackendCauses::new(storage).map_err(|cause| cause.to_string())?,
        );
        let mut parent = ParentLifecycle::new(
            run,
            launch,
            sugarsubstitute_contracts::configuration::ProfileId::new("Pair_1".into())?,
            ConnectionId::new(1)?,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
                100,
                1_000_000_000,
            )?,
            &mut causes,
        )?;
        // The authenticated Hello, Welcome and Register exchange completed.
        parent.prepared = true;
        parent.connected = true;
        parent.hello = true;
        parent.welcome = true;
        parent.registered = true;
        parent.exit = Some(DesktopExitTransaction::new(run, process)?);
        Ok(Self { parent, process, replies: VecDeque::new(), revealed: false, completed: false })
    }

    /// The real actor.
    pub(in crate::production) fn parent(&self) -> &ParentLifecycle {
        &self.parent
    }

    /// The real actor, for the coordinator's offers and close requests.
    pub(in crate::production) fn parent_mut(&mut self) -> &mut ParentLifecycle {
        &mut self.parent
    }

    /// Whether the desktop reported its window revealed.
    pub(in crate::production) fn revealed(&self) -> bool {
        self.revealed
    }

    /// Whether the desktop's final ExitCompleted was accepted.
    pub(in crate::production) fn completed(&self) -> bool {
        self.completed
    }

    /// Admit one authenticated request from the desktop at `now`, as `requests.rs` does.
    /// # Errors
    /// Returns the actor's rejection of the request.
    pub(in crate::production) fn receive(&mut self, bytes: &[u8], now: Moment) -> HarnessResult {
        let frame = FrameCodec::decode(bytes)?;
        let run = self.parent.run;
        match frame.payload {
            DecodedPayload::RestartToApply => self.parent.updates.restart_to_apply(),
            DecodedPayload::CheckForUpdates => self.parent.updates.check_requested(),
            DecodedPayload::MainRevealed(_) => self.revealed = true,
            DecodedPayload::ExitIntent { shutdown, intent, expected_code } => {
                let budget = self.parent.exit.as_mut().ok_or("Exit transaction absent")?.begin(
                    run,
                    shutdown,
                    intent,
                    i64::from(expected_code),
                    now,
                )?;
                self.parent.cleanup = Some(budget);
            }
            DecodedPayload::ExitCompleted(shutdown) => {
                self.parent
                    .exit
                    .as_mut()
                    .ok_or("Exit transaction absent")?
                    .observe(DesktopObservation::Completed { run, shutdown })
                    .map_err(|rejected| rejected.reason)?;
                self.parent.close_after_reply = Some(0);
                self.completed = true;
            }
            other => return Err(format!("The coordinator cannot take {other:?}").into()),
        }
        self.replies.push_back(frame.header);
        Ok(())
    }

    /// The desktop process exited with `code`, as the native process watch reports it.
    /// # Errors
    /// Returns the exit transaction's rejection.
    pub(in crate::production) fn exited(&mut self, code: u32) -> HarnessResult {
        let process = self.process;
        self.parent
            .exit
            .as_mut()
            .ok_or("Exit transaction absent")?
            .observe(DesktopObservation::Exited { process, code: i64::from(code) })
            .map_err(|rejected| rejected.reason)?;
        Ok(())
    }

    /// The exit transaction's view at `now`.
    /// # Errors
    /// Returns the transaction's clock rejection.
    pub(in crate::production) fn exit_snapshot(
        &mut self,
        now: Moment,
    ) -> HarnessResult<sugarsubstitute_application::supervision::DesktopExitSnapshot> {
        Ok(self.parent.exit.as_mut().ok_or("Exit transaction absent")?.poll(now)?)
    }

    /// The deadlines this actor waits for: its close transaction's and an owed close request's.
    pub(in crate::production) fn deadlines(&self) -> Vec<Moment> {
        let mut due = Vec::new();
        if let Some(budget) = self.parent.cleanup {
            due.extend([budget.graceful_until, budget.settle_until]);
        }
        if let ChildClose::Due(until) | ChildClose::Writing(until) = self.parent.child_close {
            due.push(until);
        }
        due
    }

    /// Ask the desktop to close normally, as the coordinator's close does.
    pub(in crate::production) fn ask_to_close(
        &mut self,
        now: Moment,
        budget: ShutdownBudget,
    ) -> bool {
        self.parent.ask_child_to_close(now, budget)
    }

    /// Write what the actor owes the desktop now; true when anything was written.
    /// # Errors
    /// Returns an encoding failure.
    pub(in crate::production) fn write(
        &mut self,
        wire: &WireEnd,
        now: Moment,
    ) -> HarnessResult<bool> {
        let mut wrote = false;
        while let Some(header) = self.replies.pop_front() {
            wire.send(&WireMessage {
                header,
                payload: WirePayload::Reply(ReplyOutcome::Acknowledged),
            })?;
            wrote = true;
        }
        let notice = WireHeader {
            run: self.parent.run,
            connection: Some(self.parent.connection),
            request: None,
            deadline_qpc: 1_000_000,
        };
        // `send_child_close`: an owed request goes out until its grace while the desktop lives.
        if let ChildClose::Due(until) = self.parent.child_close
            && self.parent.registered
            && !self.parent.closed
            && self.parent.cleanup.is_none()
            && now < until
        {
            wire.send(&WireMessage { header: notice, payload: WirePayload::CloseRequest })?;
            self.parent.child_close = ChildClose::Writing(until);
            self.parent.child_close.written();
            wrote = true;
        }
        // `send_update_status`: the actor's own gate; the write slot is always free here.
        if !self.parent.update_status_open() {
            return Ok(wrote);
        }
        if let Some((revision, workflow, status)) = self.parent.updates.take_due() {
            wire.send(&WireMessage {
                header: notice,
                payload: WirePayload::UpdateStatus { revision, workflow, status: &status },
            })?;
            self.parent.updates.written();
            wrote = true;
        }
        Ok(wrote)
    }
}
