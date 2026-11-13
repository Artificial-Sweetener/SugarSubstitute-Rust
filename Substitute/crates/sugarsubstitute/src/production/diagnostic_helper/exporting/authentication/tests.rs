// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rejection boundaries use inert wire facts without fabricating authenticated native origin.
use super::*;
use sugarsubstitute_contracts::lifecycle::*;
use sugarsubstitute_crash::diagnostic_client::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn facts() -> Result<(CoordinatorKey, ReportBinding), Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok((
        owner,
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Supervisor(owner)),
            IncidentRevision::new(3)?,
            ReportSessionId::new(4)?,
        ),
    ))
}
fn frame<'a>(
    owner: CoordinatorKey,
    binding: ReportBinding,
    destination: &'a [u8],
) -> Result<DiagnosticFrame<'a>, DiagnosticWireError> {
    Ok(DiagnosticFrame {
        key: DiagnosticKey::new(
            owner,
            DiagnosticRole::Reporter,
            DiagnosticSessionId::new(5).map_err(|_| DiagnosticWireError::Identity)?,
        ),
        request: DiagnosticRequestId::new(6)?,
        context: DiagnosticContext {
            incident: Some(DiagnosticIncident {
                key: binding.incident(),
                revision: binding.revision(),
            }),
            ..DiagnosticContext::default()
        },
        command: DiagnosticCommand::SaveSelectionReady {
            session: binding.session(),
            selection: DiagnosticSelection {
                id: DiagnosticRequestId::new(7)?,
                selection: ExportSelection::TextOnly,
                overwrite: false,
                destination,
            },
        },
    })
}
fn path(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[test]
fn export_original_selection_preserves_closed_content_and_identity() -> TestResult {
    let (owner, binding) = facts()?;
    let destination = path(r"D:\owned\report.txt");
    let original = frame(owner, binding, &destination)?;
    let intent = selection(original, owner, binding)?;
    assert_eq!(intent.binding(), binding);
    assert_eq!(intent.request().get(), 7);
    assert_eq!(intent.selection(), ExportSelection::TextOnly);
    let mut dump = original;
    if let DiagnosticCommand::SaveSelectionReady { selection, .. } = &mut dump.command {
        selection.selection = ExportSelection::TextAndNativeDump;
        selection.overwrite = true;
    }
    assert_eq!(selection(dump, owner, binding)?.selection(), ExportSelection::TextAndNativeDump);
    Ok(())
}
#[test]
fn export_rejects_foreign_current_session_revision_and_command_before_native_effects() -> TestResult
{
    let (owner, binding) = facts()?;
    let destination = path(r"D:\owned\report.txt");
    let original = frame(owner, binding, &destination)?;
    let mut wrong = original;
    wrong.context.incident.as_mut().ok_or("missing incident")?.revision = IncidentRevision::new(9)?;
    assert!(selection(wrong, owner, binding).is_err());
    wrong = original;
    wrong.command = DiagnosticCommand::SaveSelectionCancelled(binding.session());
    assert!(selection(wrong, owner, binding).is_err());
    wrong = original;
    if let DiagnosticCommand::SaveSelectionReady { session, .. } = &mut wrong.command {
        *session = ReportSessionId::new(9)?;
    }
    assert!(selection(wrong, owner, binding).is_err());
    wrong = original;
    wrong.key = DiagnosticKey::new(owner, DiagnosticRole::Storage, DiagnosticSessionId::new(5)?);
    assert!(selection(wrong, owner, binding).is_err());
    wrong = original;
    wrong.context.at = Some(IncidentUtc::from_unix_seconds(100)?);
    assert!(selection(wrong, owner, binding).is_err());
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([8; 16])?, ..owner };
    assert!(selection(original, foreign, binding).is_err());
    Ok(())
}
#[test]
fn export_rejects_native_destination_confusion_and_decodes_original_complete_bytes() -> TestResult {
    let (owner, binding) = facts()?;
    for value in
        [r"D:\owned\..\victim.txt", r"D:\owned\report.txt:secret", r"\?\D:\owned\report.txt"]
    {
        let destination = path(value);
        assert!(selection(frame(owner, binding, &destination)?, owner, binding).is_err());
    }
    let destination = path(r"D:\owned\report.txt");
    let original = frame(owner, binding, &destination)?;
    let mut bytes = vec![0; MAX_FRAME_BYTES];
    let count = encode(original, &mut bytes)?;
    let decoded = decode(&bytes[..count])?;
    assert_eq!(decoded, original);
    assert_eq!(selection(decoded, owner, binding)?.request().get(), 7);
    Ok(())
}

#[test]
fn export_invalid_destination_is_distinct_from_forged_role_session_owner_and_context() -> TestResult
{
    let (owner, binding) = facts()?;
    let destination = path(r"\\server\share\report.txt");
    let original = frame(owner, binding, &destination)?;
    let rejected =
        selection(original, owner, binding).err().ok_or("Destination rejection missing")?;
    assert!(destination_rejection(&rejected));
    let mut forged = original;
    forged.key = DiagnosticKey::new(owner, DiagnosticRole::Storage, DiagnosticSessionId::new(5)?);
    assert!(!destination_rejection(
        &selection(forged, owner, binding).err().ok_or("Forged role accepted")?
    ));
    forged = original;
    forged.context.incident.as_mut().ok_or("Original incident missing")?.revision =
        IncidentRevision::new(9)?;
    assert!(!destination_rejection(
        &selection(forged, owner, binding).err().ok_or("Forged revision accepted")?
    ));
    forged = original;
    forged.command = DiagnosticCommand::SaveSelectionCancelled(binding.session());
    assert!(!destination_rejection(
        &selection(forged, owner, binding).err().ok_or("Forged command accepted")?
    ));
    forged = original;
    if let DiagnosticCommand::SaveSelectionReady { session, .. } = &mut forged.command {
        *session = ReportSessionId::new(9)?;
    }
    assert!(!destination_rejection(
        &selection(forged, owner, binding).err().ok_or("Forged session accepted")?
    ));
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([8; 16])?, ..owner };
    assert!(!destination_rejection(
        &selection(original, foreign, binding).err().ok_or("Forged owner accepted")?
    ));
    forged = original;
    forged.context.at = Some(IncidentUtc::from_unix_seconds(100)?);
    assert!(!destination_rejection(
        &selection(forged, owner, binding).err().ok_or("Forged context accepted")?
    ));
    Ok(())
}
#[test]
fn export_original_authentication_and_control_rejections_never_become_destination_failures() {
    for cause in [
        ExportCause::State,
        ExportCause::Wire(DiagnosticWireError::Identity),
        ExportCause::Admission(AdmissionError::InvalidReportExportAdmission),
        ExportCause::Admission(AdmissionError::DeadlineExpired {
            deadline: Moment::from_nanos(1),
            now: Moment::from_nanos(2),
        }),
        ExportCause::Export(ReportExportCause::Rejected(
            sugarsubstitute_crash::report_export::ReportExportCode::Identity,
        )),
        ExportCause::Export(ReportExportCause::Control(AdmissionError::ScopeCapacity)),
    ] {
        assert!(!destination_rejection(&cause));
    }
}
