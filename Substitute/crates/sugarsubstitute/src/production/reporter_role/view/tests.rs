// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify transfer policy and original Core claim lifetime; no peer or frame guard is fabricated.
use super::*;
use crate::production::incident_host::tests::admitted_value;
use sugarsubstitute_contracts::lifecycle::*;
fn key() -> Result<DiagnosticKey, Box<dyn Error + Send + Sync>> {
    Ok(DiagnosticKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        DiagnosticRole::Reporter,
        DiagnosticSessionId::new(1)?,
    ))
}
fn frame(
    command: DiagnosticCommand<'_>,
) -> Result<DiagnosticFrame<'_>, Box<dyn Error + Send + Sync>> {
    let key = key()?;
    Ok(DiagnosticFrame {
        key,
        request: DiagnosticRequestId::new(1)?,
        context: DiagnosticContext {
            incident: Some(DiagnosticIncident {
                key: IncidentKey::new(IncidentSubject::Supervisor(key.owner())),
                revision: IncidentRevision::new(2)?,
            }),
            deadline: Some(DiagnosticDeadline::new(100, 10)?),
            ..DiagnosticContext::default()
        },
        command,
    })
}
#[test]
fn partial_or_wrong_offset_cannot_publish_view_and_complete_text_keeps_original_charge()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let text = b"diagnostics\n";
        use sha2::{Digest, Sha256};
        let hash: [u8; 32] = Sha256::digest(text).into();
        let session = ReportSessionId::new(1)?;
        let mut receiver = ViewReceiver::new(&context);
        assert!(matches!(
            receiver.apply_frame(
                frame(DiagnosticCommand::BeginView {
                    restart:
                        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
                    session,
                    total: text.len() as u64,
                    sha256: hash
                })?,
                &context
            )?,
            PreparedView::Partial
        ));
        assert!(matches!(
            receiver.apply_frame(
                frame(DiagnosticCommand::EndView { session, sha256: hash })?,
                &context
            ),
            Err(ViewFailure::Wire(DiagnosticWireError::Incomplete))
        ));
        let chunk = DiagnosticChunk {
            kind: DiagnosticTransferKind::View,
            total: text.len() as u64,
            offset: 1,
            sha256: hash,
            bytes: &text[..text.len() - 1],
        };
        assert!(matches!(
            receiver.apply_frame(frame(DiagnosticCommand::ViewChunk { session, chunk })?, &context),
            Err(ViewFailure::Wire(DiagnosticWireError::Capacity))
        ));
        let chunk = DiagnosticChunk { offset: 0, bytes: text, ..chunk };
        receiver.apply_frame(frame(DiagnosticCommand::ViewChunk { session, chunk })?, &context)?;
        let prepared = receiver
            .apply_frame(frame(DiagnosticCommand::EndView { session, sha256: hash })?, &context)?;
        let PreparedView::Initial(view) = prepared else {
            return Err("No complete view".into());
        };
        assert_eq!(view.text.as_bytes(), text);
        assert!(matches!(
            receiver.apply_frame(
                frame(DiagnosticCommand::BeginView {
                    restart:
                        sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
                    session,
                    total: text.len() as u64,
                    sha256: hash
                })?,
                &context
            ),
            Err(ViewFailure::Wire(DiagnosticWireError::Stale))
        ));
        let foreign =
            ReportBinding::new(view.binding.incident(), view.binding.revision().next()?, session);
        assert_eq!(receiver.presented(foreign), Err(ViewIssue::Failed));
        assert_eq!(receiver.visible, None);
        assert_eq!(receiver.presented(view.binding), Ok(()));
        assert_eq!(receiver.visible, Some(view.binding));
        Ok(view)
    })
}

#[test]
fn complete_wire_text_that_cannot_enter_native_controls_retains_original_bytes()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let text = b"diagnostics\r\n";
        use sha2::{Digest, Sha256};
        let hash: [u8; 32] = Sha256::digest(text).into();
        let session = ReportSessionId::new(1)?;
        let mut receiver = ViewReceiver::new(&context);
        receiver.apply_frame(
            frame(DiagnosticCommand::BeginView {
                restart: sugarsubstitute_contracts::incident::ReportRestartAvailability::Available,
                session,
                total: text.len() as u64,
                sha256: hash,
            })?,
            &context,
        )?;
        receiver.apply_frame(
            frame(DiagnosticCommand::ViewChunk {
                session,
                chunk: DiagnosticChunk {
                    kind: DiagnosticTransferKind::View,
                    total: text.len() as u64,
                    offset: 0,
                    sha256: hash,
                    bytes: text,
                },
            })?,
            &context,
        )?;
        assert!(matches!(
            receiver.apply_frame(
                frame(DiagnosticCommand::EndView { session, sha256: hash })?,
                &context
            ),
            Err(ViewFailure::Wire(DiagnosticWireError::Shape))
        ));
        assert_eq!(receiver.text.as_ref().map(|text| text.bytes.as_slice()), Some(text.as_slice()));
        assert_eq!(receiver.pending, None);
        assert_eq!(receiver.visible, None);
        Ok(receiver)
    })
}
#[test]
fn actual_control_keeps_original_absolute_counter_without_translating_it()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let mut receiver = ViewReceiver::new(&context);
        let original = DiagnosticDeadline::new(i64::MAX - 1, 10)?;
        let mut received = frame(DiagnosticCommand::CopyResult {
            session: ReportSessionId::new(1)?,
            result: DiagnosticActionResult::Success,
        })?;
        received.context.deadline = Some(original);
        let PreparedView::Control(actual) = receiver.apply_frame(received, &context)? else {
            return Err("actual Copy control missing".into());
        };
        assert_eq!(actual.request, received.request);
        assert_eq!(
            actual.counter,
            Some(CounterDeadline::new(original.ticks(), original.frequency())?)
        );
        assert!(matches!(actual.decision, ControlDecision::Copy(DiagnosticActionResult::Success)));
        Ok(receiver)
    })
}

#[test]
fn complete_authenticated_view_retains_unavailable_restart_and_original_charge()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        use sha2::{Digest, Sha256};
        let bytes = b"report\n";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let session = ReportSessionId::new(1)?;
        let mut receiver = ViewReceiver::new(&context);
        receiver.apply_frame(
            frame(DiagnosticCommand::BeginView {
                session,
                restart: ReportRestartAvailability::Unavailable,
                total: bytes.len() as u64,
                sha256: digest,
            })?,
            &context,
        )?;
        receiver.apply_frame(
            frame(DiagnosticCommand::ViewChunk {
                session,
                chunk: DiagnosticChunk {
                    kind: DiagnosticTransferKind::View,
                    total: bytes.len() as u64,
                    offset: 0,
                    sha256: digest,
                    bytes,
                },
            })?,
            &context,
        )?;
        let PreparedView::Initial(text) = receiver.apply_frame(
            frame(DiagnosticCommand::EndView { session, sha256: digest })?,
            &context,
        )?
        else {
            return Err("Complete unavailable view missing".into());
        };
        assert_eq!(text.restart, ReportRestartAvailability::Unavailable);
        assert_eq!(text.text, "report\n");
        Ok(text)
    })
}
