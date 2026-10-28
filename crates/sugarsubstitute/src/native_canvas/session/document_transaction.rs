// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stage one document on shared workers while retaining the preceding document for rollback.

use super::*;

/// Session-local identity of one staged document; neither a source nor a native frame identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentToken(u64);

/// Keep complete document custody and its source intent until promotion or rollback settles.
pub(super) struct RetainedDocument {
    document: Box<document::DocumentState>,
    selection: u64,
    title: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Preparing,
    Restoring,
    Promoted,
}

/// At most one candidate shares the session's execution, graphics and admission lifetime.
pub(super) struct Transaction {
    token: DocumentToken,
    previous: RetainedDocument,
    phase: Phase,
}

impl Session {
    /// Withdraw an idle document into retained custody and start an empty candidate.
    /// The caller first hides preparation from new input and continues observing old jobs.
    /// Returns `None` while old work or an earlier transaction still owns shared resources.
    ///
    /// # Errors
    /// Rejects a closing session, GPU-window presentation, or exhausted transaction identities.
    pub(crate) fn prepare_document(&mut self) -> Result<Option<DocumentToken>, Box<dyn Error>> {
        if self.closing || self.config.gpu.is_some() {
            return Err("Canvas document preparation is unavailable".into());
        }
        self.document_suspended = true;
        if self.resources.is_none()
            || self.document_transaction.is_some()
            || !self.document_work_settled()
        {
            return Ok(None);
        }
        let next = self.document_sequence.checked_add(1).ok_or("Document identity exhausted")?;
        let document = self.withdraw_document()?;
        let token = DocumentToken(next);
        self.document_sequence = next;
        self.document_transaction = Some(Transaction {
            token,
            previous: RetainedDocument {
                document,
                selection: self.selection,
                title: self.title.clone(),
            },
            phase: Phase::Preparing,
        });
        self.document_suspended = false;
        Ok(Some(token))
    }

    /// Resume the current document after a preparation request was canceled before withdrawal.
    pub(crate) fn cancel_document_preparation(&mut self) {
        self.document_suspended = false;
    }

    /// Promote only the staged document whose exact frame received native acknowledgement.
    /// Retain its predecessor until an existing disposal worker accepts complete custody.
    ///
    /// # Errors
    /// Rejects foreign tokens, canceled transactions and unacknowledged or superseded frames.
    pub(crate) fn promote_document(
        &mut self,
        token: DocumentToken,
        target: FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        let transaction = self.document_transaction.as_mut().ok_or("No staged document")?;
        if transaction.token != token
            || transaction.phase != Phase::Preparing
            || self.document.native_presented != Some(target)
            || self.document.display_evidence.is_none_or(|evidence| {
                evidence.target != target || evidence.source_revision != self.selection
            })
        {
            return Err("Staged document promotion lacks exact native evidence".into());
        }
        transaction.phase = Phase::Promoted;
        Ok(())
    }

    /// Fence a candidate and request restoration after its actual worker outcomes are observed.
    /// Cancellation does not release document, transport or pixel custody on the caller thread.
    ///
    /// # Errors
    /// Rejects foreign/promoted transactions or exhausted source identities and pacing failures.
    pub(crate) fn restore_document(&mut self, token: DocumentToken) -> Result<(), Box<dyn Error>> {
        let transaction = self.document_transaction.as_ref().ok_or("No staged document")?;
        if transaction.token != token || transaction.phase == Phase::Promoted {
            return Err("Document restoration does not own the candidate".into());
        }
        if transaction.phase == Phase::Restoring {
            return Ok(());
        }
        let next = self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        self.abandon_cpu_frame(self.navigation_time())?;
        self.source_sequence = next;
        self.selection = next;
        self.drawable = false;
        self.document_suspended = true;
        self.document_transaction.as_mut().ok_or("Document transaction disappeared")?.phase =
            Phase::Restoring;
        if let Some(Open { source: OpenSource::Acquired(owner), .. }) = &self.pending_open {
            owner.cancel();
        }
        if let Some(loading) = &self.loading {
            if let Some(owner) = &loading.acquirer {
                owner.cancel();
            }
            loading.job.cancel();
        }
        if let Some(rendering) = &mut self.rendering {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        Ok(())
    }

    /// Report actual restoration/promotion settlement without equating cancellation with disposal.
    pub(crate) fn document_transaction_pending(&self, token: DocumentToken) -> bool {
        self.document_transaction.as_ref().is_some_and(|transaction| transaction.token == token)
    }

    /// Keep canceled candidate completions from becoming product failures or new preparation.
    pub(super) fn restoring_document(&self) -> bool {
        self.document_transaction.as_ref().is_some_and(|value| value.phase == Phase::Restoring)
    }

    /// Advance the bounded transaction after completion observation, before ordinary admission.
    /// Returns true while rollback must keep ordinary document work fenced.
    pub(super) fn advance_document_transaction(&mut self) -> Result<bool, Box<dyn Error>> {
        let Some(transaction) = &self.document_transaction else { return Ok(false) };
        let phase = transaction.phase;
        if phase == Phase::Preparing {
            return Ok(false);
        }
        if self.closing || self.pending_retirement.is_some() || self.deferred_retirement.is_some() {
            return Ok(phase == Phase::Restoring);
        }
        if phase == Phase::Promoted {
            let cutoff = self.retirement_cutoff()?;
            let transaction =
                self.document_transaction.take().ok_or("Document transaction lost")?;
            self.pending_retirement = Some(Retired {
                _retained_document: Some(transaction.previous),
                ..Default::default()
            });
            self.retirement_deadline = Some(cutoff);
            return Ok(false);
        }
        if self.pending_open.is_some() {
            let cutoff = self.retirement_cutoff()?;
            self.pending_retirement =
                Some(Retired { _open: self.pending_open.take(), ..Default::default() });
            self.retirement_deadline = Some(cutoff);
            return Ok(true);
        }
        if !self.document_work_settled() {
            return Ok(true);
        }
        let cutoff = self.retirement_cutoff()?;
        let candidate = self.withdraw_document()?;
        let transaction = self.document_transaction.take().ok_or("Document transaction lost")?;
        self.document = transaction.previous.document;
        self.document_suspended = false;
        self.selection = transaction.previous.selection;
        self.title = transaction.previous.title;
        if let Some(cause) = self.failed.take() {
            tracing::debug!(operation="canvas_document_restored", cause=%cause,
                "Restored the preceding document after candidate failure");
        }
        self.pending_retirement =
            Some(Retired { _document: Some(candidate), ..Default::default() });
        self.retirement_deadline = Some(cutoff);
        Ok(true)
    }

    /// Require terminal document work and settled native acquisition before moving custody.
    fn document_work_settled(&self) -> bool {
        self.initializing.is_none()
            && self.loading.is_none()
            && self.pending_open.is_none()
            && self.rendering.is_none()
            && self.displaying.is_none()
            && self.presenting.is_none()
            && self.acquisition_retirement.is_none()
            && self.document.cpu_pending.is_none()
            && self.document.pending_cpu_evidence.is_none()
            && self.document.pending_gpu_evidence.is_none()
            && self.awaiting_gpu.is_none()
            && self.pending_retirement.is_none()
            && self.deferred_retirement.is_none()
            && self.retirement.is_none()
    }

    /// Transfer any retained predecessor with the closing document to the existing worker.
    pub(super) fn take_retained_document(&mut self) -> Option<RetainedDocument> {
        self.document_transaction.take().map(|transaction| transaction.previous)
    }
}
