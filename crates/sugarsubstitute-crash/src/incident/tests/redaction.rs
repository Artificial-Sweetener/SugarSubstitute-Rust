// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual typed allowlist and distinguish output marker bytes from omitted source.
use super::*;

#[test]
fn allowlist_preserves_categorical_fault_and_exit_without_paths() -> Result<(), Box<dyn Error>> {
    let chunk = RedactedTextChunk::from_facts(&[
        RedactedDiagnosticFact::Failure(FailureEvidence::NativeFault {
            code: 0xc0000005,
            address: 4096,
        }),
        RedactedDiagnosticFact::Exit(ExitAttribution::Desktop {
            kind: DesktopExitKind::Unexpected,
            status: 0,
        }),
        RedactedDiagnosticFact::Capture(CaptureEvidence::Unavailable(CaptureAbsence::Missing)),
    ])?;
    let text = std::str::from_utf8(chunk.as_bytes())?;
    assert!(text.contains("0xc0000005"));
    assert!(text.contains("Unexpected"));
    assert!(text.contains("Missing"));
    assert_eq!(chunk.original_bytes(), chunk.as_bytes().len() as u64);
    assert_eq!(chunk.omitted_bytes(), 0);
    Ok(())
}
#[test]
fn bounded_tail_counts_source_omission_separately_from_marker() -> Result<(), Box<dyn Error>> {
    let facts = vec![
        RedactedDiagnosticFact::Log {
            code: IncidentLogCode::StaleResultRejected,
            sequence: u64::MAX
        };
        2048
    ];
    let chunk = RedactedTextChunk::from_facts(&facts)?;
    let text = std::str::from_utf8(chunk.as_bytes())?;
    assert!(chunk.as_bytes().len() <= 65_536);
    assert!(text.ends_with("[additional diagnostic facts omitted]\n"));
    assert!(chunk.omitted_bytes() > 0);
    assert!(chunk.original_bytes() - chunk.omitted_bytes() < chunk.as_bytes().len() as u64);
    assert!(RedactedTextChunk::from_facts(&[]).is_err());
    assert!(RedactedTextChunk::from_facts(&vec![facts[0]; 2049]).is_err());
    Ok(())
}

#[test]
fn external_json_errors_redact_owner_output_and_preserve_original_cause()
-> Result<(), Box<dyn Error>> {
    let text = String::from_utf8(encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?)?;
    for (sentinel, changed) in [
        (
            "PRIVATE_JSON_FIELD_8ca312",
            text.replace(
                "\"schema_version\":3",
                "\"schema_version\":3,\"PRIVATE_JSON_FIELD_8ca312\":0",
            ),
        ),
        (
            "PRIVATE_JSON_VARIANT_d41bf7",
            text.replace("\"unexplained\"", "\"PRIVATE_JSON_VARIANT_d41bf7\""),
        ),
    ] {
        let error =
            decode_summary(changed.as_bytes()).err().ok_or("Malformed JSON was accepted")?;
        let cause = error.source().ok_or("Original JSON cause was lost")?;
        assert!(cause.downcast_ref::<serde_json::Error>().is_some());
        assert!(!cause.to_string().contains(sentinel));
        assert!(cause.to_string().contains("Unknown incident"));
        let json = cause.downcast_ref::<serde_json::Error>().ok_or("JSON type lost")?;
        assert!(json.line() > 0 && json.column() > 0);
        assert!(!error.to_string().contains(sentinel));
        assert!(!format!("{error:?}").contains(sentinel));
        assert!(!format!("{error:#?}").contains(sentinel));
    }
    Ok(())
}
