// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed restart presentation and rejection data before any action privilege.
use super::{super::*, support::*};
use sugarsubstitute_contracts::incident::*;
#[test]
fn restart_availability_is_canonical_authenticated_schema2_data() -> TestResult {
    let session = binding()?.session();
    for restart in [ReportRestartAvailability::Unavailable, ReportRestartAvailability::Available] {
        let frame = reporter(
            1,
            DiagnosticCommand::BeginView { session, restart, total: 1, sha256: [7; 32] },
            true,
        )?;
        let bytes = encoded(frame)?;
        assert_eq!(decode(&bytes)?, frame);
        let mut old = bytes.clone();
        old[4] = 1;
        assert_eq!(decode(&old), Err(DiagnosticWireError::Schema));
        let mut bad = bytes;
        let at = bad.len() - 41;
        bad[at] = 2;
        assert_eq!(decode(&bad), Err(DiagnosticWireError::Schema));
    }
    Ok(())
}
#[test]
fn restart_rejection_preserves_exact_request_and_view_without_acknowledgement_fields() -> TestResult
{
    let frame =
        reporter(9, DiagnosticCommand::RestartUnavailable { session: binding()?.session() }, true)?;
    let bytes = encoded(frame)?;
    assert_eq!(decode(&bytes)?, frame);
    assert_eq!(DiagnosticRequestKind::of(frame.command)?, DiagnosticRequestKind::from_code(32)?);
    let mut wrong = frame;
    wrong.key = key(DiagnosticRole::Storage)?;
    let mut output = [0xa5; MAX_HEADER_BYTES];
    assert_eq!(encode(wrong, &mut output), Err(DiagnosticWireError::Role));
    assert!(output.iter().all(|v| *v == 0xa5));
    wrong = frame;
    wrong.context.deadline = None;
    assert_eq!(encode(wrong, &mut output), Err(DiagnosticWireError::Shape));
    wrong = frame;
    wrong.context.incident = None;
    assert_eq!(encode(wrong, &mut output), Err(DiagnosticWireError::Shape));
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(decode(&trailing), Err(DiagnosticWireError::Shape));
    Ok(())
}
