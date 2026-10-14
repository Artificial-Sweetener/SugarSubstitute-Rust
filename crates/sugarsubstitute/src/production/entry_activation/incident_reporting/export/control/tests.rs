// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inert result facts exercise the real wire gate without constructing authentication or export proof.
use super::*;
#[test]
fn exporting_native_control_requires_core_counter_for_same_original_wire_request() -> Result<()> {
    let owner = sugarsubstitute_contracts::lifecycle::CoordinatorKey {
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
        epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
    };
    let key = DiagnosticKey::new(owner, DiagnosticRole::Reporter, DiagnosticSessionId::new(3)?);
    let binding = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(4)?,
        ReportSessionId::new(5)?,
    );
    let original_wire = DiagnosticRequestId::new(7)?;
    let native_selection = DiagnosticRequestId::new(9)?;
    let original_until = Moment::from_nanos(47);
    let control = input(
        key,
        original_wire,
        binding,
        original_until,
        DiagnosticActionResult::Failed(ReportFailure::Export),
    );
    assert_eq!(control.key, key);
    assert_eq!(control.request, original_wire);
    assert_ne!(control.request, native_selection);
    assert_eq!(control.until, original_until);
    assert_eq!(control.session, binding.session());
    assert!(control.context.deadline.is_none());
    let ReporterControl::Export(result) = control.command else {
        return Err("Export result command missing".into());
    };
    let frame = sugarsubstitute_crash::diagnostic_client::DiagnosticFrame {
        key: control.key,
        request: control.request,
        context: control.context,
        command:
            sugarsubstitute_crash::diagnostic_client::DiagnosticCommand::ExportSelectionResult {
                session: control.session,
                result,
            },
    };
    let mut bytes = [0; 512];
    assert_eq!(
        sugarsubstitute_crash::diagnostic_client::encode(frame, &mut bytes),
        Err(sugarsubstitute_crash::diagnostic_client::DiagnosticWireError::Shape)
    );
    Ok(())
}
