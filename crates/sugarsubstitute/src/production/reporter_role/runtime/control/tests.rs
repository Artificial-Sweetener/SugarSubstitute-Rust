// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check inert control metadata at the actual receiving owner without fabricating native tokens.
use super::*;
use sugarsubstitute_contracts::lifecycle::*;
fn control() -> Result<ControlResult, Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(ControlResult {
        request: DiagnosticRequestId::new(1)?,
        incident: DiagnosticIncident {
            key: IncidentKey::new(IncidentSubject::Supervisor(owner)),
            revision: IncidentRevision::new(2)?,
        },
        session: ReportSessionId::new(1)?,
        decision: ControlDecision::Copy(DiagnosticActionResult::Success),
        counter: Some(CounterDeadline::new(100, 10)?),
    })
}
#[test]
fn control_kind_counter_and_exact_request_fence_native_copy()
-> Result<(), Box<dyn std::error::Error>> {
    let mut received = control()?;
    let binding =
        ReportBinding::new(received.incident.key, received.incident.revision, received.session);
    let command = ReportCommand::new(binding, ReportAction::CopyText);
    let wire = received.request;
    assert!(received.validate(command, wire).is_ok());
    for action in [
        ReportAction::Export(ExportSelection::TextOnly),
        ReportAction::Dismiss,
        ReportAction::DismissAndRequestRestart,
    ] {
        assert!(received.validate(ReportCommand::new(binding, action), wire).is_err());
    }
    received.decision = ControlDecision::Export(DiagnosticActionResult::Success);
    assert!(received.validate(command, wire).is_err());
    assert!(
        received
            .validate(
                ReportCommand::new(binding, ReportAction::Export(ExportSelection::TextOnly)),
                wire
            )
            .is_ok()
    );
    received.decision = ControlDecision::Copy(DiagnosticActionResult::Success);
    assert!(received.validate(command, DiagnosticRequestId::new(2)?).is_err());
    received.session = ReportSessionId::new(2)?;
    assert!(received.validate(command, wire).is_err());
    received.session = binding.session();
    received.incident.revision = IncidentRevision::new(3)?;
    assert!(received.validate(command, wire).is_err());
    received.incident.revision = binding.revision();
    received.counter = None;
    assert!(received.validate(command, wire).is_err());
    received.counter = Some(CounterDeadline::new(100, 10)?);
    assert!(received.validate(command, wire).is_ok());
    assert_eq!(received.counter, Some(CounterDeadline::new(100, 10)?));
    Ok(())
}
#[test]
fn unrequested_actual_control_is_retained_and_emits_no_surface_update()
-> Result<(), Box<dyn std::error::Error>> {
    let role = ReporterRole::parse(
        [
            "--internal-production-reporter",
            "--bootstrap",
            "1",
            "--until-qpc",
            "100",
            "--qpc-frequency",
            "10",
            "--locale",
            "en",
        ]
        .map(std::ffi::OsString::from),
    )?;
    let mut session = ReporterSession::new(role);
    let original = control()?;
    let request = original.request;
    assert!(session.control(original).is_err());
    assert!(session.update.is_none());
    assert!(session.pending.is_none());
    assert_eq!(session.rejected_control.as_ref().map(|control| control.request), Some(request));
    assert!(session.control(control()?).is_err());
    assert_eq!(session.rejected_control.as_ref().map(|control| control.request), Some(request));
    Ok(())
}
#[test]
fn close_wire_header_and_actual_control_require_same_fresh_acknowledgement()
-> Result<(), Box<dyn std::error::Error>> {
    let mut received = control()?;
    let original =
        ReportBinding::new(received.incident.key, received.incident.revision, received.session);
    let fresh = IncidentRevision::new(original.revision().get() + 1)?;
    received.incident.revision = fresh;
    let at = IncidentUtc::from_unix_seconds(20)?;
    let acknowledgement = DurableAcknowledgement::new(original.incident(), fresh, at);
    let frame = DiagnosticFrame {
        key: DiagnosticKey::new(
            original.incident().subject().coordinator(),
            DiagnosticRole::Reporter,
            DiagnosticSessionId::new(1)?,
        ),
        request: received.request,
        context: DiagnosticContext {
            incident: Some(received.incident),
            deadline: Some(DiagnosticDeadline::new(100, 10)?),
            ..DiagnosticContext::default()
        },
        command: DiagnosticCommand::CloseAfterDurableAck {
            session: received.session,
            acknowledgement,
        },
    };
    let mut bytes = [0_u8; MAX_HEADER_BYTES];
    let length = encode(frame, &mut bytes)?;
    let actual = ControlResult::from_frame(decode(&bytes[..length])?)?
        .ok_or("decoded Close control missing")?;
    let dismiss = ReportCommand::new(original, ReportAction::Dismiss);
    assert!(actual.validate(dismiss, received.request).is_ok());
    assert!(
        actual
            .validate(ReportCommand::new(original, ReportAction::CopyText), received.request)
            .is_err()
    );
    for revision in [original.revision(), IncidentRevision::new(fresh.get() + 1)?] {
        let mismatched = DiagnosticFrame {
            command: DiagnosticCommand::CloseAfterDurableAck {
                session: received.session,
                acknowledgement: DurableAcknowledgement::new(original.incident(), revision, at),
            },
            ..frame
        };
        assert!(matches!(encode(mismatched, &mut bytes), Err(DiagnosticWireError::Identity)));
    }
    let mut wrong = control()?;
    wrong.incident.revision = fresh;
    wrong.decision = ControlDecision::Close(DurableAcknowledgement::new(
        original.incident(),
        original.revision(),
        at,
    ));
    assert!(wrong.validate(dismiss, received.request).is_err());
    wrong.incident.revision = original.revision();
    assert!(wrong.validate(dismiss, received.request).is_err());
    let foreign = IncidentKey::new(IncidentSubject::Supervisor(CoordinatorKey {
        namespace: NamespaceId::new([9; 32]),
        epoch: CoordinatorEpoch::new([8; 16])?,
    }));
    let foreign_frame = DiagnosticFrame {
        command: DiagnosticCommand::CloseAfterDurableAck {
            session: received.session,
            acknowledgement: DurableAcknowledgement::new(foreign, fresh, at),
        },
        ..frame
    };
    assert!(matches!(encode(foreign_frame, &mut bytes), Err(DiagnosticWireError::Identity)));
    wrong.incident.revision = fresh;
    wrong.decision = ControlDecision::Close(DurableAcknowledgement::new(foreign, fresh, at));
    assert!(wrong.validate(dismiss, received.request).is_err());
    Ok(())
}
#[test]
fn actual_pending_copy_keeps_first_wrong_wire_control_and_emits_no_surface_action()
-> Result<(), Box<dyn std::error::Error>> {
    let metadata = control()?;
    let current =
        ReportBinding::new(metadata.incident.key, metadata.incident.revision, metadata.session);
    let old = ReportBinding::new(
        current.incident(),
        IncidentRevision::new(current.revision().get() - 1)?,
        current.session(),
    );
    sugarsubstitute_desktop::report::copy_fixture::with_pending_report_copy(
        old,
        current,
        |request, command| -> Result<(), Box<dyn std::error::Error>> {
            let role = ReporterRole::parse(
                [
                    "--internal-production-reporter",
                    "--bootstrap",
                    "1",
                    "--until-qpc",
                    "100",
                    "--qpc-frequency",
                    "10",
                    "--locale",
                    "en",
                ]
                .map(std::ffi::OsString::from),
            )?;
            let mut session = ReporterSession::new(role);
            let wire = DiagnosticRequestId::new(11)?;
            let pending = (request, command, wire);
            session.pending = Some(pending);
            session.current = Some(current);
            let frame = DiagnosticFrame {
                key: DiagnosticKey::new(
                    current.incident().subject().coordinator(),
                    DiagnosticRole::Reporter,
                    DiagnosticSessionId::new(current.session().get())?,
                ),
                request: wire,
                context: DiagnosticContext {
                    incident: Some(metadata.incident),
                    deadline: Some(DiagnosticDeadline::new(100, 10)?),
                    ..DiagnosticContext::default()
                },
                command: DiagnosticCommand::ExportSelectionResult {
                    session: current.session(),
                    result: DiagnosticActionResult::Success,
                },
            };
            let mut bytes = [0_u8; MAX_HEADER_BYTES];
            let length = encode(frame, &mut bytes)?;
            let wrong = ControlResult::from_frame(decode(&bytes[..length])?)?
                .ok_or("wrong-kind decoded control missing")?;
            assert_eq!(
                session.control(wrong).err().ok_or("wrong kind accepted")?.operation(),
                "reporter_result_action_kind"
            );
            assert_eq!(session.pending, Some(pending));
            assert!(session.update.is_none());
            assert_eq!(session.current, Some(current));
            let retained =
                session.rejected_control.as_ref().ok_or("first rejected observation lost")?;
            assert_eq!(retained.request, wire);
            assert_eq!(retained.counter, Some(CounterDeadline::new(100, 10)?));
            assert!(matches!(
                retained.decision,
                ControlDecision::Export(DiagnosticActionResult::Success)
            ));
            let corrected = DiagnosticFrame {
                command: DiagnosticCommand::CopyResult {
                    session: current.session(),
                    result: DiagnosticActionResult::Success,
                },
                ..frame
            };
            let length = encode(corrected, &mut bytes)?;
            let copy = ControlResult::from_frame(decode(&bytes[..length])?)?
                .ok_or("Copy decoded control missing")?;
            assert!(session.control(copy).is_err());
            assert_eq!(session.pending, Some(pending));
            assert!(session.update.is_none());
            assert!(matches!(
                session.rejected_control.as_ref().ok_or("first observation changed")?.decision,
                ControlDecision::Export(DiagnosticActionResult::Success)
            ));
            Ok(())
        },
    )??;
    Ok(())
}

#[test]
fn authenticated_restart_rejection_resolves_only_matching_intent_without_ack_or_close()
-> Result<(), Box<dyn std::error::Error>> {
    let metadata = control()?;
    let current =
        ReportBinding::new(metadata.incident.key, metadata.incident.revision, metadata.session);
    let old = ReportBinding::new(
        current.incident(),
        IncidentRevision::new(current.revision().get() - 1)?,
        current.session(),
    );
    sugarsubstitute_desktop::report::copy_fixture::with_pending_report_restart(
        old,
        current,
        |request, command| -> Result<(), Box<dyn std::error::Error>> {
            let wire = DiagnosticRequestId::new(11)?;
            let frame = DiagnosticFrame {
                key: DiagnosticKey::new(
                    current.incident().subject().coordinator(),
                    DiagnosticRole::Reporter,
                    DiagnosticSessionId::new(current.session().get())?,
                ),
                request: wire,
                context: DiagnosticContext {
                    incident: Some(metadata.incident),
                    deadline: Some(DiagnosticDeadline::new(100, 10)?),
                    ..DiagnosticContext::default()
                },
                command: DiagnosticCommand::RestartUnavailable { session: current.session() },
            };
            let mut bytes = [0u8; MAX_HEADER_BYTES];
            let length = encode(frame, &mut bytes)?;
            let rejected =
                ControlResult::from_frame(decode(&bytes[..length])?)?.ok_or("Rejection missing")?;
            assert!(rejected.validate(command, wire).is_ok());
            assert!(
                rejected
                    .validate(ReportCommand::new(current, ReportAction::Dismiss), wire)
                    .is_err()
            );
            assert!(rejected.validate(command, DiagnosticRequestId::new(12)?).is_err());
            let role = ReporterRole::parse(
                [
                    "--internal-production-reporter",
                    "--bootstrap",
                    "1",
                    "--until-qpc",
                    "100",
                    "--qpc-frequency",
                    "10",
                    "--locale",
                    "en",
                ]
                .map(std::ffi::OsString::from),
            )?;
            let mut session = ReporterSession::new(role);
            session.pending = Some((request, command, wire));
            session.current = Some(current);
            session.control(rejected)?;
            assert!(session.pending.is_none());
            assert_eq!(session.current, Some(current));
            assert!(
                matches!(session.update,Some(ReportSurfaceUpdate::RestartUnavailable { request:actual,binding }) if actual==request && binding==current)
            );
            Ok(())
        },
    )??;
    Ok(())
}
