// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize closed decoder traversal, compatibility and retained bounded values.
use super::*;
use serde_json::Value;

fn codec_cause(error: &IncidentStoreError, expected: &str) {
    assert!(matches!(error, IncidentStoreError::Codec(_)));
    let cause = error.source().map(ToString::to_string).unwrap_or_default();
    assert!(cause.contains(expected), "unexpected cause: {cause}");
}
// Test reordering uses the genuine parser/serializer, never a production JSON grammar.
fn tag_last(value: &Value) -> Result<String, Box<dyn Error>> {
    Ok(match value {
        Value::Object(fields) => {
            let is_tag = |name: &str| {
                matches!(
                    name,
                    "subject"
                        | "mechanism"
                        | "availability"
                        | "disposition"
                        | "result"
                        | "outcome"
                        | "record"
                )
            };
            let mut entries = Vec::new();
            for tags in [false, true] {
                for (name, value) in fields.iter().rev().filter(|(name, _)| is_tag(name) == tags) {
                    entries.push(format!("{}:{}", serde_json::to_string(name)?, tag_last(value)?));
                }
            }
            format!("{{{}}}", entries.join(","))
        }
        Value::Array(entries) => {
            let entries = entries.iter().map(tag_last).collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", entries.join(","))
        }
        _ => serde_json::to_string(value)?,
    })
}
fn reordered(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(tag_last(&serde_json::from_slice(bytes)?)?.into_bytes())
}
#[test]
fn unknown_payload_array_rejects_before_64038_byte_value_traversal() -> Result<(), Box<dyn Error>> {
    use super::super::codec_evidence::PayloadRecord;
    let closed =
        format!("{{\"disposition\":\"unavailable\",\"junk\":[{}]}}", vec!["0"; 32000].join(","));
    assert_eq!(closed.len(), 64038);
    for text in [closed, "{\"disposition\":\"unavailable\",\"junk\":[".into()] {
        let error = super::super::codec_parse::parse::<PayloadRecord>(text.as_bytes())
            .err()
            .ok_or("junk accepted")?;
        codec_cause(&error.into(), "Unknown incident field");
    }
    Ok(())
}
#[test]
fn duplicate_key_and_unknown_escaped_identifier_reject_before_malformed_value()
-> Result<(), Box<dyn Error>> {
    use super::super::codec_evidence::PayloadRecord;
    let cases = [
        (r#"{"disposition":"unavailable","disposition":["#, "duplicate field `disposition`"),
        (r#"{"disposition":"unavailable","\u0050RIVATE_ESCAPED":["#, "Unknown incident field"),
    ];
    for (text, cause) in cases {
        let error: IncidentStoreError =
            super::super::codec_parse::parse::<PayloadRecord>(text.as_bytes())
                .err()
                .ok_or("invalid accepted")?
                .into();
        codec_cause(&error, cause);
        assert!(!format!("{error} {error:?}").contains("PRIVATE_ESCAPED"));
    }
    Ok(())
}
#[test]
fn all_nine_tagged_owners_accept_late_tags_and_reverse_known_field_order()
-> Result<(), Box<dyn Error>> {
    let seed = seed()?;
    let bytes = encode_seed(&seed)?;
    assert_eq!(decode_seed(&reordered(&bytes)?)?, seed);
    let base = summary()?;
    let summary = IncidentSummary::new(
        base.seed().clone(),
        base.revision(),
        SummaryFacts::new(
            base.facts().retention(),
            base.facts().exit(),
            FailureEvidence::NativeFault { code: 0xc0000005, address: 42 },
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Discarded(DiscardedPayload::new(
                PayloadDiscard::UserRequested,
                64,
            )?),
            ReportDisposition::new(1, Some(ReportAttempt::Failed(ReportFailure::Presentation)))?,
            SymbolOutcome::NotRequested,
        ),
    )?;
    let stored = StoredIncidentSummary::new(summary, None)?;
    let bytes = encode_summary(&stored)?;
    assert_eq!(encode_summary(&decode_summary(&reordered(&bytes)?)?)?, bytes);
    let outcome =
        SymbolOutcome::Unresolved(UnresolvedSymbols::new(SymbolFailure::MissingDump, None)?);
    let bytes = encode_symbol_report(&outcome)?;
    assert_eq!(decode_symbol_report(&reordered(&bytes)?)?, outcome);
    for discarded in [false, true] {
        let state = IncidentStoreState::CapturePromotion(Box::new(
            super::capture_codec::capture_plan(discarded)?,
        ));
        let bytes = encode_store_state(&state)?;
        assert_eq!(encode_store_state(&decode_store_state(&reordered(&bytes)?)?)?, bytes);
    }
    Ok(())
}
#[test]
fn all_store_shapes_preserve_flattened_fields_with_content_before_schema()
-> Result<(), Box<dyn Error>> {
    let migration = super::migration::journal()?;
    let states = [
        IncidentStoreState::Ledger(SummaryEvictions::new(owner(2)?.namespace, 0, 0, 0, None)?),
        IncidentStoreState::SummaryMigration(Box::new(migration.clone())),
        IncidentStoreState::SummaryMigrationArchive(Box::new(SummaryMigrationArchive::new(
            migration,
        )?)),
        IncidentStoreState::PayloadDiscard(Box::new(super::retention::journal()?)),
    ];
    for state in states {
        let bytes = encode_store_state(&state)?;
        assert_eq!(encode_store_state(&decode_store_state(&reordered(&bytes)?)?)?, bytes);
    }
    Ok(())
}
#[test]
fn shared_failure_code_preserves_string_and_number_types_with_tag_last()
-> Result<(), Box<dyn Error>> {
    use super::super::codec_evidence::FailureRecord;
    for (text, expected) in [
        (
            r#"{"code":"presentation","mechanism":"startup"}"#,
            FailureEvidence::Startup(
                sugarsubstitute_contracts::startup::StartupFailureCode::Presentation,
            ),
        ),
        (
            r#"{"address":42,"code":4294967295,"mechanism":"native_fault"}"#,
            FailureEvidence::NativeFault { code: u32::MAX, address: 42 },
        ),
    ] {
        assert_eq!(serde_json::from_str::<FailureRecord>(text)?.into_facts(), expected);
    }
    for text in [
        r#"{"code":0,"mechanism":"startup"}"#,
        r#"{"code":"presentation","address":42,"mechanism":"native_fault"}"#,
        r#"{"code":4294967296,"address":42,"mechanism":"native_fault"}"#,
    ] {
        assert!(serde_json::from_str::<FailureRecord>(text).is_err());
    }
    Ok(())
}
#[test]
fn required_nullable_raw_frame_distinguishes_absence_and_null() -> Result<(), Box<dyn Error>> {
    let accepted = br#"{"schema_version":1,"content":{"raw_frame":null,"failure":"missing_dump","outcome":"unresolved"}}"#;
    assert_eq!(
        decode_symbol_report(accepted)?,
        SymbolOutcome::Unresolved(UnresolvedSymbols::new(SymbolFailure::MissingDump, None)?)
    );
    let missing =
        br#"{"schema_version":1,"content":{"failure":"missing_dump","outcome":"unresolved"}}"#;
    codec_cause(
        &decode_symbol_report(missing).err().ok_or("missing accepted")?,
        "missing field `raw_frame`",
    );
    Ok(())
}
#[test]
fn rejected_domain_text_preserves_facts_and_complete_json_error_priority()
-> Result<(), Box<dyn Error>> {
    let bytes = encode_seed(&seed()?)?;
    let text = String::from_utf8(bytes)?.replace("build_1", &"x".repeat(65));
    let error = decode_seed(text.as_bytes()).err().ok_or("oversized build accepted")?;
    assert!(matches!(error, IncidentStoreError::Facts(IncidentError::InvalidText)));
    assert!(error.source().and_then(|cause| cause.downcast_ref::<IncidentError>()).is_some());
    let malformed = format!("{} trailing", text);
    codec_cause(
        &decode_seed(malformed.as_bytes()).err().ok_or("trailing accepted")?,
        "trailing characters",
    );
    let zero_revision = text.replace("\"revision\":1", "\"revision\":0");
    assert!(matches!(
        decode_seed(zero_revision.as_bytes()),
        Err(IncidentStoreError::Facts(IncidentError::InvalidIdentity))
    ));
    Ok(())
}
#[test]
fn maximum_domain_text_unicode_and_escaped_text_preserve_exact_encoder_bytes()
-> Result<(), Box<dyn Error>> {
    let release = ReleaseSymbols::new(
        BuildIdentity::new(&"b".repeat(64))?,
        FileLabel::new(&"m".repeat(128))?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([7; 16], 1)?,
        [8; 32],
        [9; 32],
    );
    let outcome = SymbolOutcome::Resolved(FailingFrame::new(
        DumpId::new([3; 16])?,
        release,
        1000,
        5,
        SymbolName::new(&"\u{e9}".repeat(256))?,
        Some(SourceLine::new(FileLabel::new(&"s".repeat(128))?, 15)?),
    ));
    let bytes = encode_symbol_report(&outcome)?;
    let escaped = String::from_utf8(bytes.clone())?.replace("\u{e9}", "\\u00e9");
    assert_eq!(decode_symbol_report(escaped.as_bytes())?, outcome);
    assert_eq!(encode_symbol_report(&decode_symbol_report(&reordered(&bytes)?)?)?, bytes);
    Ok(())
}
#[test]
fn bounded_payload_vector_never_pushes_ninth_typed_attachment() -> Result<(), Box<dyn Error>> {
    use super::super::codec_evidence::PayloadRecord;
    let attachment = format!(
        r#"{{"kind":"log","retained_bytes":1,"original_bytes":1,"omitted_bytes":0,"sha256":{}}}"#,
        serde_json::to_string(&[1; 32])?
    );
    for count in [1, 8] {
        let text = format!(
            r#"{{"attachments":[{}],"disposition":"retained"}}"#,
            vec![attachment.as_str(); count].join(",")
        );
        let PayloadRecord::Retained { attachments } = serde_json::from_str(&text)? else {
            return Err("wrong payload".into());
        };
        assert_eq!(attachments.len(), count);
        assert_eq!(attachments.capacity(), 8);
    }
    let text = format!(
        r#"{{"attachments":[{}],"disposition":"retained"}}"#,
        [attachment.as_str(); 9].join(",")
    );
    let error = serde_json::from_str::<PayloadRecord>(&text).err().ok_or("nine accepted")?;
    assert!(error.to_string().contains("attachment inventory exceeds its bound"));
    Ok(())
}

#[test]
fn escaped_unknown_tag_retains_original_cause_without_exposing_text() -> Result<(), Box<dyn Error>>
{
    let bytes = br#"{"schema_version":1,"content":{"outcome":"\u0050RIVATE_TAG"}}"#;
    let error = decode_symbol_report(bytes).err().ok_or("unknown tag accepted")?;
    codec_cause(&error, "Unknown incident variant");
    assert!(!format!("{error} {error:?}").contains("PRIVATE_TAG"));
    Ok(())
}
