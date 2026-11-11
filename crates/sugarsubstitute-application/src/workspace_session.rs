// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Order desktop checkpoint capture, publication and bounded closing without adapter effects.
//!
//! The composing owner authenticates the page and backend independently, performs storage on
//! execution-owned workers, and retains the page until [`WorkspaceSession::close_ready`].
//! A publication acknowledgement establishes durable state; capture alone never does.
mod bindings;
mod capture;
pub mod close_save;
mod schedule;
#[cfg(test)]
mod tests;

pub use capture::{CaptureTicket, DurableCheckpoint};
pub use schedule::AutosavePolicy;
use schedule::Schedule;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    desktop_session::{
        CheckpointId, SessionError, admit_operation, same_fence, validate_message,
        wire::{SessionCapabilities, SessionFence, SessionMessage, SessionOperation, SessionScope},
    },
    execution::{Moment, TimeError},
};

/// A refused transition leaves the previously acknowledged checkpoint intact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    /// The portable protocol or a complete revision fence was refused.
    Contract(SessionError),
    /// A clock deadline cannot be represented.
    Clock(TimeError),
    /// A newer or unassociated on-disk original inhibits publication.
    Protected,
    /// Loading, reconciliation or another capture/publication still owns this transition.
    Busy,
    /// A durable revision would exceed the shared JavaScript-safe integer bound.
    Exhausted,
}
impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Workspace checkpoint transition refused: {self:?}")
    }
}
impl Error for WorkspaceError {}
impl From<SessionError> for WorkspaceError {
    fn from(value: SessionError) -> Self {
        Self::Contract(value)
    }
}
impl From<TimeError> for WorkspaceError {
    fn from(value: TimeError) -> Self {
        Self::Clock(value)
    }
}

/// The latest admitted page observation, independent of the host's durable revision counter.
struct Frontend {
    fence: SessionFence,
    capabilities: SessionCapabilities,
    committed: bool,
    persistence: bool,
}

/// One profile/backend workspace's synchronous policy; scheduling remains with execution.
pub struct WorkspaceSession {
    scope: SessionScope,
    origin: String,
    frontend: Frontend,
    loaded: bool,
    reconciled: bool,
    protected: bool,
    next_revision: u64,
    durable: Option<DurableCheckpoint>,
    schedule: Schedule,
    active: Option<CaptureTicket>,
    publishing: bool,
    closing: Option<Moment>,
    close_failed: bool,
    /// Oldest change carried by the running capture, until its publication is durable.
    unsaved_since: Option<Moment>,
}
impl WorkspaceSession {
    /// Bind independently trusted scope/origin and the host-granted fresh webview UUID.
    /// No page data may select these values. Begin loading before admitting capture.
    /// # Errors
    /// Refuses malformed scope/origin/webview before retaining any session.
    pub fn new(
        scope: SessionScope,
        origin: String,
        webview: String,
    ) -> Result<Self, WorkspaceError> {
        let frontend = Frontend {
            fence: SessionFence { webview, revision: 0, bindings: vec![] },
            capabilities: SessionCapabilities {
                capture: false,
                materialize: false,
                chunk_transfer: false,
                artifact_resolution: false,
            },
            committed: false,
            persistence: true,
        };
        let probe = SessionMessage {
            protocol: "sugarcubes.desktop-session".into(),
            version: 1,
            request_id: frontend.fence.webview.clone(),
            scope: scope.clone(),
            origin: origin.clone(),
            operation: SessionOperation::Ready {
                fence: frontend.fence.clone(),
                capabilities: frontend.capabilities.clone(),
                committed: false,
                workflow_persistence: true,
            },
        };
        validate_message(&probe, &scope, &origin)?;
        Ok(Self {
            scope,
            origin,
            frontend,
            loaded: false,
            reconciled: false,
            protected: false,
            next_revision: 0,
            durable: None,
            schedule: Schedule::default(),
            active: None,
            publishing: false,
            closing: None,
            close_failed: false,
            unsaved_since: None,
        })
    }

    /// Complete the one startup load. Unsupported originals must set `protected` even if a
    /// previous checkpoint could be restored. `durable` comes only from verified store bytes.
    /// # Errors
    /// Rejects repeated completion or a revision outside the portable counter range.
    pub fn loaded(
        &mut self,
        durable: Option<DurableCheckpoint>,
        protected: bool,
    ) -> Result<(), WorkspaceError> {
        if self.loaded {
            return Err(WorkspaceError::Busy);
        }
        self.next_revision = durable.as_ref().map_or(0, |value| value.revision);
        if self.next_revision > capture::MAX_REVISION {
            return Err(WorkspaceError::Exhausted);
        }
        self.reconciled = durable.is_none();
        self.durable = durable;
        self.protected = protected;
        self.loaded = true;
        Ok(())
    }

    /// Admit a complete committed observation from the authenticated current page. Equal
    /// observation revisions are immutable; they cannot change bindings or readiness.
    /// # Errors
    /// Refuses foreign origin/scope/webview, backwards revisions and changes after close.
    pub fn observe(&mut self, message: &SessionMessage, now: Moment) -> Result<(), WorkspaceError> {
        if self.close_ready(now) {
            return Err(WorkspaceError::Busy);
        }
        validate_message(message, &self.scope, &self.origin)?;
        admit_operation(
            message,
            &self.frontend.capabilities,
            self.frontend.committed,
            self.frontend.persistence,
            &self.frontend.fence,
        )?;
        let SessionOperation::Ready { fence, capabilities, committed, workflow_persistence } =
            &message.operation
        else {
            return Err(SessionError::Invalid.into());
        };
        if fence.revision > self.frontend.fence.revision {
            if self.reconciled {
                bindings::admit(&self.frontend.fence, fence)?;
            }
            self.schedule.changed(now)?;
            self.frontend = Frontend {
                fence: fence.clone(),
                capabilities: capabilities.clone(),
                committed: *committed,
                persistence: *workflow_persistence,
            };
            if self.closing.is_some() {
                self.schedule.force(now);
            }
        }
        Ok(())
    }

    /// Finish materialization only for the current complete committed page observation.
    /// # Errors
    /// Refuses stale runtime/entry/generation/revision bindings or an uncommitted page.
    pub fn reconciled(&mut self, fence: &SessionFence) -> Result<(), WorkspaceError> {
        if !self.loaded || !self.frontend.committed || !same_fence(fence, &self.frontend.fence) {
            return Err(SessionError::Stale.into());
        }
        self.reconciled = true;
        Ok(())
    }

    /// Request a checkpoint after native placement/input state changes.
    /// # Errors
    /// Refuses clock overflow or mutations after close has frozen the final capture.
    pub fn changed(&mut self, now: Moment) -> Result<(), WorkspaceError> {
        if self.closing.is_some() {
            return Err(WorkspaceError::Busy);
        }
        self.schedule.changed(now)?;
        Ok(())
    }

    /// Include a final independently observed native sample while the original close is pending.
    /// # Errors
    /// Refuses absent/expired close, preserving its original cutoff.
    pub fn sampled_during_close(&mut self, now: Moment) -> Result<(), WorkspaceError> {
        if self.closing.is_none_or(|cutoff| now >= cutoff) {
            return Err(WorkspaceError::Busy);
        }
        self.schedule.force(now);
        Ok(())
    }

    /// Earliest autosave or original final-flush deadline, without allocating a timer.
    pub fn deadline(&self) -> Option<Moment> {
        let due = (self.active.is_none() && !self.protected && self.live() && self.committed())
            .then(|| self.schedule.deadline())
            .flatten();
        due.into_iter().chain(self.closing).min()
    }

    /// Borrow the last store-acknowledged checkpoint, never an unpersisted capture.
    pub fn durable(&self) -> Option<&DurableCheckpoint> {
        self.durable.as_ref()
    }
    /// Preserve loaded originals after failed startup replay. The page remains usable, while
    /// this lifetime cannot publish its initial/default workspace over the saved checkpoint.
    pub fn protect_originals(&mut self) {
        self.protected = true;
    }

    /// Borrow the complete latest admitted page observation without granting native authority.
    pub fn fence(&self) -> &SessionFence {
        &self.frontend.fence
    }

    /// Whether the current page has reached its real committed persistence-enabled boundary.
    pub fn committed(&self) -> bool {
        self.frontend.committed && self.frontend.persistence
    }

    /// Whether startup load/materialization has released normal checkpoint capture.
    pub fn live(&self) -> bool {
        self.loaded && self.reconciled
    }

    /// Whether the final store result or original cutoff permits native close dispatch.
    /// Execution jobs still require observed retirement after this policy allows close.
    pub fn close_ready(&self, now: Moment) -> bool {
        self.closing.is_some_and(|cutoff| {
            now >= cutoff || self.close_failed || (!self.schedule.dirty() && self.active.is_none())
        })
    }

    /// What the close kept, once close is ready at `now`; `None` while the final save runs.
    /// Nothing is unsaved when the page never reached a live committed persistence-enabled
    /// boundary, because the durable checkpoint on disk is then the latest authored state. A
    /// failed final publication is a storage failure; unsaved work at the cutoff is stale, or
    /// protected when unsupported originals forbid any publication.
    pub fn close_verdict(&self, now: Moment) -> Option<close_save::SaveVerdict> {
        use close_save::{SaveFailure, SaveVerdict};
        let cutoff = self.closing?;
        if !self.close_ready(now) {
            return None;
        }
        if self.close_failed {
            return Some(SaveVerdict::Failed(SaveFailure::Storage));
        }
        let unsaved = self.schedule.dirty() || self.active.is_some();
        if !unsaved || !self.live() || !self.committed() {
            return Some(SaveVerdict::Saved);
        }
        if self.protected {
            return Some(SaveVerdict::Failed(SaveFailure::Protected));
        }
        let since = self.unsaved_since.or(self.schedule.since()).unwrap_or(cutoff);
        Some(SaveVerdict::Stale { since: since.min(now) })
    }

    /// Freeze the final committed state while the original frontend/native peer still exists.
    /// Repeated calls tighten, never renew, the same original shutdown budget.
    /// # Errors
    /// Refuses an already-expired cutoff; no capture or publication is manufactured.
    pub fn begin_close(&mut self, now: Moment, cutoff: Moment) -> Result<(), WorkspaceError> {
        if let Some(original) = self.closing {
            // A later budget only tightens the original cutoff, even when it already passed.
            self.closing = Some(original.min(cutoff));
            return Ok(());
        }
        if cutoff <= now {
            return Err(SessionError::Stale.into());
        }
        self.closing = Some(self.closing.map_or(cutoff, |original| original.min(cutoff)));
        self.schedule.force(now);
        Ok(())
    }
}
