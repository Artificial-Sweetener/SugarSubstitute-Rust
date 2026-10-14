// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream the actual charged immutable report with one unchanged context and native end operation.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Default)]
struct Delivery {
    end: Option<LifecyclePipeOperationId>,
    written: bool,
    presented: bool,
}
impl Delivery {
    fn written(&mut self, operation: LifecyclePipeOperationId) {
        if self.end == Some(operation) {
            self.written = true;
        }
    }
    fn complete(&self) -> bool {
        self.end.is_some() && self.written && self.presented
    }
}
enum Phase {
    Begin,
    Chunk(usize),
    End,
    Presentation,
}
pub(super) struct Stream {
    prepared: PreparedReportView,
    plan: ReporterViewPlan,
    digest: Option<[u8; 32]>,
    next_request: u64,
    phase: Phase,
    pending: Option<LifecyclePipeOperationId>,
    delivery: Delivery,
    admitted: Option<Moment>,
}
impl Stream {
    pub(super) fn validate(
        view: &PreparedReportView,
        plan: &ReporterViewPlan,
    ) -> Result<(), ReporterSourceIssue> {
        let binding = view.view().binding();
        let expected = DiagnosticIncident { key: binding.incident(), revision: binding.revision() };
        if plan.key.role() != DiagnosticRole::Reporter
            || plan.context.incident != Some(expected)
            || plan.context.deadline.is_none()
            || plan.context.operation.is_some()
            || plan.context.at.is_some()
            || plan.previous.is_some_and(|old| old >= binding.revision())
            || plan
                .first_request
                .get()
                .checked_add(2 + view.view().text().len().div_ceil(MAX_CHUNK_BYTES) as u64)
                .is_none()
        {
            return Err(ReporterSourceIssue::Identity);
        }
        Ok(())
    }
    pub(super) fn new(prepared: PreparedReportView, plan: ReporterViewPlan) -> Self {
        let next_request = plan.first_request.get();
        Self {
            prepared,
            plan,
            digest: None,
            next_request,
            phase: Phase::Begin,
            pending: None,
            delivery: Delivery::default(),
            admitted: None,
        }
    }
    pub(super) fn until(&self) -> Moment {
        self.plan.until
    }
    /// Retain the first effective Core cutoff after ancestor scope bounding; never renew per chunk.
    pub(super) fn latch(&mut self, until: Moment) -> Result<(), ReporterSourceIssue> {
        if self.admitted.is_some_and(|original| original != until) {
            return Err(ReporterSourceIssue::Identity);
        }
        self.admitted = Some(until);
        self.plan.until = until;
        Ok(())
    }
    pub(super) fn delivered(&self) -> bool {
        self.delivery.complete()
    }
    pub(super) fn written(&mut self, operation: LifecyclePipeOperationId) {
        self.delivery.written(operation);
        if self.pending == Some(operation) {
            self.pending = None;
        }
    }
    /// Observe only a strictly decoded frame received through this parent's authenticated channel.
    pub(super) fn presentation(&mut self, frame: DiagnosticFrame<'_>) {
        let binding = self.prepared.view().binding();
        let expected = DiagnosticIncident { key: binding.incident(), revision: binding.revision() };
        let command = match self.plan.previous {
            None => DiagnosticCommand::Presented(binding.session()),
            Some(_) => DiagnosticCommand::ViewReplaced(binding.session()),
        };
        if frame.key == self.plan.key
            && frame.context.incident == Some(expected)
            && frame.context.operation.is_none()
            && frame.context.deadline.is_none()
            && frame.context.at.is_none()
            && frame.command == command
            && self.delivery.end.is_some()
        {
            self.delivery.presented = true;
        }
    }
    /// Encode at most one bounded chunk on the real finite Core worker; accepted is not Written.
    pub(super) fn send(&mut self, parent: &mut ReporterParent) -> Result<(), ReporterSourceIssue> {
        if self.pending.is_some() || matches!(self.phase, Phase::Presentation) {
            return Ok(());
        }
        let view = self.prepared.view();
        let digest =
            *self.digest.get_or_insert_with(|| Sha256::digest(view.text().as_bytes()).into());
        let session = view.binding().session();
        let bytes = view.text().as_bytes();
        let command = match self.phase {
            Phase::Begin => match self.plan.previous {
                None => DiagnosticCommand::BeginView {
                    session,
                    total: bytes.len() as u64,
                    sha256: digest,
                },
                Some(old) => DiagnosticCommand::ReplaceView {
                    session,
                    old,
                    total: bytes.len() as u64,
                    sha256: digest,
                },
            },
            Phase::Chunk(offset) => DiagnosticCommand::ViewChunk {
                session,
                chunk: DiagnosticChunk {
                    kind: DiagnosticTransferKind::View,
                    total: bytes.len() as u64,
                    offset: offset as u64,
                    sha256: digest,
                    bytes: &bytes[offset..(offset + MAX_CHUNK_BYTES).min(bytes.len())],
                },
            },
            Phase::End => DiagnosticCommand::EndView { session, sha256: digest },
            Phase::Presentation => return Ok(()),
        };
        let request = DiagnosticRequestId::new(self.next_request)
            .map_err(|_| ReporterSourceIssue::Identity)?;
        let frame =
            DiagnosticFrame { key: self.plan.key, request, context: self.plan.context, command };
        let Some(operation) =
            parent.send(self.plan.until, frame).map_err(|_| ReporterSourceIssue::Parent)?
        else {
            return Ok(());
        };
        self.pending = Some(operation);
        self.next_request =
            self.next_request.checked_add(1).ok_or(ReporterSourceIssue::Identity)?;
        self.phase = match self.phase {
            Phase::Begin => {
                if bytes.is_empty() {
                    Phase::End
                } else {
                    Phase::Chunk(0)
                }
            }
            Phase::Chunk(offset) => {
                let next = (offset + MAX_CHUNK_BYTES).min(bytes.len());
                if next == bytes.len() { Phase::End } else { Phase::Chunk(next) }
            }
            Phase::End => {
                self.delivery.end = Some(operation);
                Phase::Presentation
            }
            Phase::Presentation => return Err(ReporterSourceIssue::Identity),
        };
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_end_written_and_presentation_are_both_required_in_either_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let end = LifecyclePipeOperationId::new(19)?;
        let foreign = LifecyclePipeOperationId::new(20)?;
        let mut proof = Delivery { end: Some(end), ..Delivery::default() };
        proof.presented = true;
        assert!(!proof.complete());
        proof.written(foreign);
        assert!(!proof.complete());
        proof.written(end);
        assert!(proof.complete());
        let mut reverse = Delivery { end: Some(end), ..Delivery::default() };
        reverse.written(end);
        assert!(!reverse.complete());
        reverse.presented = true;
        assert!(reverse.complete());
        Ok(())
    }
    #[test]
    fn unaccepted_end_never_delivers_even_with_other_observations()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut proof = Delivery { presented: true, ..Delivery::default() };
        proof.written(LifecyclePipeOperationId::new(1)?);
        assert!(!proof.complete());
        Ok(())
    }
}
