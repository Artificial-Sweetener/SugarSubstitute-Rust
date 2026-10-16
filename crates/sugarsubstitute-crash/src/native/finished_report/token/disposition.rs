// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact protected completion and every native disposition field before settlement.
use super::*;
use crate::incident::{
    CaptureDispositionCompletion, CaptureDispositionPlan, CaptureRecordedFileFacts,
};
use sugarsubstitute_contracts::incident::{AttachmentKind, IncidentRevision};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticCaptureFence, DiagnosticObject, DiagnosticReadFacts,
};
impl Token {
    pub(crate) fn dispose(
        &mut self,
        plan: &CaptureDispositionPlan,
        payload_revision: IncidentRevision,
        fence: &mut DiagnosticCaptureFence<'_>,
    ) -> Result<(), Issue> {
        self.dispose_current(plan, payload_revision, fence)
    }
    /// Move the original cause only at the protected-source trait's actual terminal boundary.
    pub(in crate::native::finished_report) fn settlement_failure(
        &mut self,
        issue: Issue,
    ) -> super::super::SettlementFailure {
        let cause = self.fields.as_mut().and_then(|fields| fields.state.failures.first.take());
        super::super::SettlementFailure::new(issue, cause)
    }
    fn dispose_current(
        &mut self,
        plan: &CaptureDispositionPlan,
        payload_revision: IncidentRevision,
        fence: &mut DiagnosticCaptureFence<'_>,
    ) -> Result<(), Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        fields.check(fence.context())?;
        let recovering = fields.phase == Phase::Unused;
        if (!recovering && (fields.phase != Phase::Issued || fields.state.token == 0))
            || fields.operation != fence.operation()
            || (recovering == (fields.operation == plan.historical_operation()))
            || !fence.matches_generation(&fields.generation)
        {
            return Err(Issue::Identity);
        }
        let prepared = if recovering {
            preparation(plan)
        } else {
            *fields.state.preparation.as_ref().ok_or(Issue::Identity)?
        };
        let observed = (|| {
            let (journal, journal_file) = fence.journal()?;
            let (summary, summary_file) = fence.summary()?;
            let payload = fence.payload()?;
            Ok::<_, sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError>((
                journal,
                journal_file,
                summary,
                summary_file,
                payload,
            ))
        })()
        .map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let (journal, journal_file, summary, summary_file, payload) = observed;
        let CaptureDispositionCompletion::Promoted {
            summary_file: expected_summary,
            payload_file: expected_payload,
        } = plan.completion()
        else {
            return Err(Issue::Identity);
        };
        let Some((payload_object, payload_file)) = payload else {
            return Err(Issue::Identity);
        };
        if journal != DiagnosticObject::Journal(plan.historical_operation())
            || summary != DiagnosticObject::Summary(plan.incident())
            || payload_object
                != DiagnosticObject::Payload(
                    plan.incident(),
                    payload_revision,
                    AttachmentKind::Minidump,
                )
            || !same(summary_file, expected_summary)
            || !same(payload_file, expected_payload)
            || prepared.subject != calls::subject(plan.incident())
            || prepared.uuid != *plan.uuid().bytes()
            || prepared.build_length as usize != plan.build().as_str().len()
            || prepared.build.get(..prepared.build_length as usize)
                != Some(plan.build().as_str().as_bytes())
            || !recorded_same(&prepared.source, plan.source())
            || !recorded_same(&prepared.metadata, plan.metadata_preimage())
            || !recorded_same(&prepared.original_stage.file, plan.original_stage())
            || !recorded_same(&prepared.candidate_stage.file, plan.candidate_stage())
            || prepared.database_identity != plan.database_identity()
            || prepared.metrics_identity != plan.metrics_identity()
        {
            return Err(Issue::Identity);
        }
        let context = fence.context().clone();
        let guard = fields.generation.borrow(&context).map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let identities = guard.identities().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let owner = guard.coordinator().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let origin = guard.origin().map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        let recovery_origin = abi::RecoveryOrigin::from_origin(origin, fields.operation)
            .map_err(|_| Issue::Identity)?;
        if identities != [prepared.database_identity, prepared.metrics_identity]
            || owner != plan.capture_generation()
            || origin != plan.capture_origin()
        {
            return Err(Issue::Identity);
        }
        let mut check = || fence.check_pinned_current();
        let mut current =
            ControlInvocation::for_fenced_generation(&guard, &mut fields.state.history, &mut check);
        let permit = abi::Disposition {
            prepared,
            disposition: 1,
            reason: 0,
            revision: plan.incident_revision().get(),
            summary: file(summary_file),
            has_payload: 1,
            reserved_payload: 0,
            payload: file(payload_file),
            intent: file(journal_file),
            plan_sha256: journal_file.sha256(),
            current: operation(fields.operation),
            authorization: if recovering { 2 } else { 1 },
            reserved_authorization: 0,
        };
        let input = abi::Discard {
            call: abi::Call {
                version: 1,
                phase: 1,
                lease: fields.state.token,
                original: fields.state.original.counter,
                current: current.source(),
            },
            permit: &permit,
        };
        let mut output = abi::DispositionOutput {
            state: abi::Output::empty(),
            source: 0,
            metadata: 0,
            stages_absent: 0,
            reserved: 0,
        };
        if recovering {
            let database = guard
                .database()
                .map_err(|cause| {
                    fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
                    Issue::Current
                })?
                .as_raw_handle() as usize;
            let metrics = guard
                .metrics()
                .map_err(|cause| {
                    fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
                    Issue::Current
                })?
                .as_raw_handle() as usize;
            let input = abi::Recover {
                version: 1,
                reserved: 0,
                database,
                metrics,
                database_identity: identities[0],
                metrics_identity: identities[1],
                permit: &permit,
                original: fields.state.original.counter,
                current: current.source(),
            };
            fields.query = Some(validation::Query::from_plan(plan));
            fields.state.preparation = Some(prepared);
            fields.phase = Phase::Issued;
            // SAFETY: the actual retained historical generation, genuine current fence,
            // preheld fixed roles and same PIN-backed ABI own every synchronous borrow.
            if let Some(origin) = recovery_origin {
                let input = abi::Recover2 { version: 2, reserved: 0, origin, disposition: input };
                // SAFETY: exact origin and genuine current journal/source fences remain borrowed.
                unsafe { (fields.state.api.recover2)(&input, &mut output) };
            } else {
                // SAFETY: original strict Coordinator origin retains its supported v1 ABI.
                unsafe { (fields.state.api.recover)(&input, &mut output) };
            }
        } else {
            // SAFETY: the same real pinned API/token, actual immutable completion pins,
            // preheld CPP/Rust roles and synchronous exclusive current/output borrows.
            if let Some(origin) = recovery_origin {
                let input = abi::Discard2 { version: 2, reserved: 0, origin, disposition: input };
                // SAFETY: same Root-local token, exact full origin and actual completion fence.
                unsafe { (fields.state.api.discard2)(&input, &mut output) };
            } else {
                // SAFETY: original strict Coordinator origin retains the supported v1 path.
                unsafe { (fields.state.api.discard)(&input, &mut output) };
            }
        }
        let control = current.take_failure();
        drop(current);
        fields.observe(&output.state, control)?;
        if !valid_disposition_output(&output, recovering, prepared.source.bytes) {
            fields.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        fields.accept_retirement(&output.state);
        if output.state.status != 0 {
            return Err(Issue::Native);
        }
        Ok(())
    }
}
/// Accept native destruction only with complete checked disposition or explicit recovery no-owner.
pub(super) fn valid_disposition_output(
    output: &abi::DispositionOutput,
    recovering: bool,
    source_bytes: u64,
) -> bool {
    let state = &output.state;
    if output.reserved != 0
        || !(1..=4).contains(&output.source)
        || !(1..=3).contains(&output.metadata)
        || output.stages_absent > 3
        || state.read_bytes != 0
        || state.reclaimed_bytes > source_bytes
    {
        return false;
    }
    if state.status == 0 {
        return state.retired == 1
            && state.lease == 0
            && state.handles == 0
            && state.locks == 0
            && (recovering || state.facts_valid == 1)
            && state.primary_error == 0
            && state.cleanup_count == 0
            && state.omitted_cleanup == 0
            && matches!(output.source, 2 | 3)
            && output.metadata == 2
            && output.stages_absent == 3
            && state.reclaimed_bytes == if output.source == 2 { source_bytes } else { 0 };
    }
    state.retired == 0
        || (recovering
            && matches!(state.status, 6 | 7)
            && state.primary_error != 0
            && output.source == 4
            && output.metadata == 3
            && output.stages_absent == 0
            && state.reclaimed_bytes == 0
            && state.facts_valid == 0)
}
fn operation(key: sugarsubstitute_contracts::incident::LogicalStoreOperationKey) -> abi::Operation {
    abi::Operation {
        namespace: *key.owner().namespace.bytes(),
        epoch: *key.owner().epoch.bytes(),
        id: key.id().get(),
    }
}
fn preparation(plan: &CaptureDispositionPlan) -> abi::Preparation {
    let mut build = [0; 64];
    build[..plan.build().as_str().len()].copy_from_slice(plan.build().as_str().as_bytes());
    let recorded = |facts: CaptureRecordedFileFacts| abi::FileFacts {
        identity: facts.identity(),
        bytes: facts.bytes(),
        sha256: facts.sha256(),
    };
    abi::Preparation {
        version: 1,
        reserved: 0,
        historical: operation(plan.historical_operation()),
        subject: calls::subject(plan.incident()),
        uuid: *plan.uuid().bytes(),
        build_length: plan.build().as_str().len() as u32,
        build,
        reserved_build: 0,
        database_identity: plan.database_identity(),
        metrics_identity: plan.metrics_identity(),
        metadata: recorded(plan.metadata_preimage()),
        source: recorded(plan.source()),
        original_stage: abi::StageFacts {
            role: 1,
            reserved: 0,
            file: recorded(plan.original_stage()),
        },
        candidate_stage: abi::StageFacts {
            role: 2,
            reserved: 0,
            file: recorded(plan.candidate_stage()),
        },
    }
}
fn file(facts: DiagnosticReadFacts) -> abi::FileFacts {
    abi::FileFacts { identity: facts.file_identity(), bytes: facts.bytes(), sha256: facts.sha256() }
}
fn same(actual: DiagnosticReadFacts, expected: CaptureRecordedFileFacts) -> bool {
    actual.file_identity() == expected.identity()
        && actual.bytes() == expected.bytes()
        && actual.sha256() == expected.sha256()
}
fn recorded_same(actual: &abi::FileFacts, expected: CaptureRecordedFileFacts) -> bool {
    actual.identity == expected.identity()
        && actual.bytes == expected.bytes()
        && actual.sha256 == expected.sha256()
}
