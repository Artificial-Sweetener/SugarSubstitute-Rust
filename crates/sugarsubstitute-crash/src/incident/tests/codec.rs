// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify persisted grammar, full historical identity, arithmetic and explicit unavailable evidence.
use super::*;

#[test]
fn seed_summary_roundtrip_preserves_unexplained_native_zero() -> Result<(), Box<dyn Error>> {
    let seed = seed()?;
    assert_eq!(decode_seed(&encode_seed(&seed)?)?, seed);
    let summary = StoredIncidentSummary::new(summary()?, None)?;
    let decoded = decode_summary(&encode_summary(&summary)?)?;
    assert_eq!(decoded, summary);
    assert_eq!(
        decoded.summary().facts().exit(),
        ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 }
    );
    Ok(())
}
#[test]
fn strict_fields_nullable_fields_and_versions_are_required() -> Result<(), Box<dyn Error>> {
    let bytes = encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?;
    let text = String::from_utf8(bytes)?;
    for changed in [
        text.replace("\"schema_version\":2", "\"schema_version\":3"),
        text.replace("\"schema_version\":2", "\"schema_version\":2,\"unknown\":true"),
        text.replace("\"acknowledged\":null,", ""),
        text.replace("\"last\":null", "\"last\":null,\"extra\":0"),
        text.replace("\"unexplained\"", "\"invented\""),
    ] {
        assert!(decode_summary(changed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn duplicate_fields_trailing_content_and_oversize_are_rejected() -> Result<(), Box<dyn Error>> {
    let bytes = encode_seed(&seed()?)?;
    let text = String::from_utf8(bytes)?;
    assert!(
        decode_seed(text.replace("\"revision\":1", "\"revision\":1,\"revision\":1").as_bytes())
            .is_err()
    );
    assert!(decode_seed(format!("{text} {{}}").as_bytes()).is_err());
    assert!(matches!(decode_seed(&vec![b' '; 4097]), Err(IncidentStoreError::RecordBound)));
    assert!(decode_seed(text.replace("\"generation\":4", "\"generation\":0").as_bytes()).is_err());
    Ok(())
}
#[test]
fn summary_cross_facts_and_timestamps_are_checked() -> Result<(), Box<dyn Error>> {
    let text = String::from_utf8(encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?)?;
    assert!(
        decode_summary(text.replace("\"occurred\":101", "\"occurred\":99").as_bytes()).is_err()
    );
    assert!(
        decode_summary(text.replace("\"acknowledged\":null", "\"acknowledged\":100").as_bytes())
            .is_err()
    );
    assert!(
        decode_summary(
            text.replace("\"created\":100", "\"created\":18446744073709551615").as_bytes()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn exact_release_and_single_frame_roundtrip() -> Result<(), Box<dyn Error>> {
    let release = ReleaseSymbols::new(
        BuildIdentity::new("build_1")?,
        FileLabel::new("host.exe")?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([7; 16], 1)?,
        [8; 32],
        [9; 32],
    );
    assert_eq!(decode_release_symbols(&encode_release_symbols(&release)?)?, release);
    let outcome = SymbolOutcome::Resolved(FailingFrame::new(
        DumpId::new([3; 16])?,
        release,
        1000,
        5,
        SymbolName::new("実際の関数")?,
        Some(SourceLine::new(FileLabel::new("host.rs")?, 15)?),
    ));
    assert_eq!(decode_symbol_report(&encode_symbol_report(&outcome)?)?, outcome);
    let text = String::from_utf8(encode_symbol_report(&outcome)?)?;
    assert!(decode_symbol_report(text.replace("\"line\":15", "\"line\":0").as_bytes()).is_err());
    assert!(
        decode_symbol_report(text.replace("\"host.rs\"", "\"../host.rs\"").as_bytes()).is_err()
    );
    Ok(())
}
#[test]
fn missing_symbol_evidence_cannot_gain_raw_frame() -> Result<(), Box<dyn Error>> {
    let outcome = SymbolOutcome::Unresolved(UnresolvedSymbols::new(
        SymbolFailure::MissingSymbols,
        Some(RawFaultFrame::new(DumpId::new([3; 16])?, FileLabel::new("host.exe")?, 1000, 5)),
    )?);
    let text = String::from_utf8(encode_symbol_report(&outcome)?)?;
    assert_eq!(decode_symbol_report(text.as_bytes())?, outcome);
    assert!(
        decode_symbol_report(text.replace("missing_symbols", "missing_dump").as_bytes()).is_err()
    );
    Ok(())
}
#[test]
fn eviction_journal_retains_historical_full_operation_and_exact_increment()
-> Result<(), Box<dyn Error>> {
    let original = SummaryEvictions::new(owner(2)?.namespace, 0, 0, 0, None)?;
    let candidate = original.record(SummaryEviction::new(
        seed()?.key(),
        IncidentUtc::from_unix_seconds(200)?,
        SummaryEvictionKind::Pending,
    ))?;
    let original_bytes = encode_store_state(&IncidentStoreState::Ledger(original))?;
    let candidate_bytes = encode_store_state(&IncidentStoreState::Ledger(candidate))?;
    let journal = SummaryEvictionJournal::new(
        LogicalStoreOperationKey::new(owner(5)?, IncidentOperationId::new(1)?),
        IncidentRevision::new(2)?,
        IncidentFileFacts::new([6; 24], [7; 32], 100)?,
        IncidentFileFacts::new(
            [8; 24],
            super::super::codec_ledger::digest(&original_bytes),
            original_bytes.len() as u64,
        )?,
        IncidentFileFacts::new(
            [9; 24],
            super::super::codec_ledger::digest(&candidate_bytes),
            candidate_bytes.len() as u64,
        )?,
        original,
        candidate,
    )?;
    for stage in [
        IncidentFileFacts::new([9; 24], [10; 32], candidate_bytes.len() as u64)?,
        IncidentFileFacts::new(
            [9; 24],
            journal.candidate_sha256(),
            candidate_bytes.len() as u64 + 1,
        )?,
        IncidentFileFacts::new([8; 24], journal.candidate_sha256(), candidate_bytes.len() as u64)?,
    ] {
        assert!(
            SummaryEvictionJournal::new(
                journal.operation(),
                journal.revision(),
                journal.victim(),
                journal.original_file(),
                stage,
                original,
                candidate
            )
            .is_err()
        );
    }
    let state = IncidentStoreState::SummaryEviction(Box::new(journal));
    let bytes = encode_store_state(&state)?;
    assert_eq!(decode_store_state(&bytes)?, state);
    let text = String::from_utf8(bytes)?;
    assert!(decode_store_state(text.replace("\"pending\":1", "\"pending\":2").as_bytes()).is_err());
    assert!(
        decode_store_state(text.replace("\"revision\":2", "\"revision\":0").as_bytes()).is_err()
    );
    Ok(())
}
#[test]
fn ledger_overflow_keeps_original() -> Result<(), Box<dyn Error>> {
    let event = SummaryEviction::new(
        seed()?.key(),
        IncidentUtc::from_unix_seconds(200)?,
        SummaryEvictionKind::Pending,
    );
    let original = SummaryEvictions::new(owner(2)?.namespace, u64::MAX, 0, 0, Some(event))?;
    assert!(original.record(event).is_err());
    assert_eq!(
        decode_store_state(&encode_store_state(&IncidentStoreState::Ledger(original))?)?,
        IncidentStoreState::Ledger(original)
    );
    Ok(())
}

#[test]
fn all_format_bounds_apply_before_parse() {
    let oversized = vec![b' '; 65_537];
    assert!(matches!(decode_summary(&oversized), Err(IncidentStoreError::RecordBound)));
    assert!(matches!(decode_store_state(&oversized), Err(IncidentStoreError::RecordBound)));
    assert!(matches!(decode_release_symbols(&oversized), Err(IncidentStoreError::RecordBound)));
    assert!(matches!(
        decode_symbol_report(&vec![b' '; 262_145]),
        Err(IncidentStoreError::RecordBound)
    ));
}

#[test]
fn nested_unit_variants_reject_unknown_fields() -> Result<(), Box<dyn Error>> {
    let text = String::from_utf8(encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?)?;
    for changed in [
        text.replace(
            "\"mechanism\":\"unexplained\"",
            "\"mechanism\":\"unexplained\",\"unexpected\":1",
        ),
        text.replace(
            "\"outcome\":\"not_requested\"",
            "\"outcome\":\"not_requested\",\"unexpected\":1",
        ),
        text.replace(
            "\"disposition\":\"unavailable\"",
            "\"disposition\":\"unavailable\",\"unexpected\":1",
        ),
    ] {
        assert!(decode_summary(changed.as_bytes()).is_err());
    }
    Ok(())
}

#[test]
fn zero_epoch_and_stage_subject_mismatch_are_rejected() -> Result<(), Box<dyn Error>> {
    let text = String::from_utf8(encode_seed(&seed()?)?)?;
    assert!(
        decode_seed(
            text.replace("\"stage\":\"startup\"", "\"stage\":\"supervisor_initialization\"")
                .as_bytes()
        )
        .is_err()
    );
    let expected = format!("\"epoch\":{:?}", [2_u8; 16]).replace(' ', "");
    let zero = format!("\"epoch\":{:?}", [0_u8; 16]).replace(' ', "");
    assert!(decode_seed(text.replace(&expected, &zero).as_bytes()).is_err());
    Ok(())
}

#[test]
fn malformed_nested_nullable_and_architecture_do_not_default() -> Result<(), Box<dyn Error>> {
    let release = ReleaseSymbols::new(
        BuildIdentity::new("build_1")?,
        FileLabel::new("host.exe")?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([7; 16], 1)?,
        [8; 32],
        [9; 32],
    );
    let outcome = SymbolOutcome::Resolved(FailingFrame::new(
        DumpId::new([3; 16])?,
        release.clone(),
        1000,
        5,
        SymbolName::new("function")?,
        None,
    ));
    let text = String::from_utf8(encode_symbol_report(&outcome)?)?;
    assert!(decode_symbol_report(text.replace(",\"source\":null", "").as_bytes()).is_err());
    assert!(decode_symbol_report(text.replace("x86_64", "arm64").as_bytes()).is_err());
    assert!(
        decode_release_symbols(
            String::from_utf8(encode_release_symbols(&release)?)?
                .replace("\"pdb_age\":1", "\"pdb_age\":0")
                .as_bytes()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn retained_text_counts_markers_separately_and_rejects_duplicate_kinds()
-> Result<(), Box<dyn Error>> {
    let baseline = summary()?;
    let payload = PayloadDisposition::Retained(AttachmentInventory::new(&[AttachmentFacts::new(
        AttachmentKind::Log,
        256,
        1000,
        800,
        [4; 32],
    )?])?);
    let facts = baseline.facts();
    let retained = IncidentSummary::new(
        baseline.seed().clone(),
        baseline.revision(),
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            payload,
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?;
    let retained = StoredIncidentSummary::new(retained.clone(), Some(retained.revision()))?;
    let text = String::from_utf8(encode_summary(&retained)?)?;
    assert_eq!(decode_summary(text.as_bytes())?, retained);
    assert!(
        decode_summary(text.replace("\"omitted_bytes\":800", "\"omitted_bytes\":1001").as_bytes())
            .is_err()
    );
    assert!(
        decode_summary(
            text.replace("\"retained_bytes\":256", "\"retained_bytes\":262145").as_bytes()
        )
        .is_err()
    );
    let attachment = "{\"kind\":\"log\",\"retained_bytes\":256,\"original_bytes\":1000,\"omitted_bytes\":800,\"sha256\":[4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4]}";
    assert!(
        decode_summary(text.replace(attachment, &format!("{attachment},{attachment}")).as_bytes())
            .is_err()
    );
    assert!(
        decode_summary(text.replace(attachment, &[attachment; 9].join(",")).as_bytes()).is_err()
    );
    Ok(())
}

#[test]
fn wrong_release_dump_and_actual_fault_address_fail_summary_validation()
-> Result<(), Box<dyn Error>> {
    let baseline = summary()?;
    let dump = DumpId::new([3; 16])?;
    let release = ReleaseSymbols::new(
        baseline.seed().build().clone(),
        FileLabel::new("host.exe")?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([7; 16], 1)?,
        [8; 32],
        [9; 32],
    );
    let facts = baseline.facts();
    let resolved = IncidentSummary::new(
        baseline.seed().clone(),
        baseline.revision(),
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            FailureEvidence::NativeFault { code: 1, address: 4096 },
            CaptureEvidence::Finished(dump),
            facts.payload().clone(),
            facts.reporting(),
            SymbolOutcome::Resolved(FailingFrame::new(
                dump,
                release,
                4096,
                10,
                SymbolName::new("actual")?,
                None,
            )),
        ),
    )?;
    let resolved = StoredIncidentSummary::new(resolved, None)?;
    let text = String::from_utf8(encode_summary(&resolved)?)?;
    assert_eq!(decode_summary(text.as_bytes())?, resolved);
    assert!(
        decode_summary(
            text.replace(
                "\"address\":4096,\"module_offset\":10",
                "\"address\":4097,\"module_offset\":10"
            )
            .as_bytes()
        )
        .is_err()
    );
    assert!(
        decode_summary(
            text.replace("\"build\":\"build_1\",\"module\"", "\"build\":\"build_2\",\"module\"")
                .as_bytes()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn symbol_and_ledger_unknown_versions_are_preserved_as_rejection() -> Result<(), Box<dyn Error>> {
    let ledger = SummaryEvictions::new(owner(2)?.namespace, 0, 0, 0, None)?;
    let text = String::from_utf8(encode_store_state(&IncidentStoreState::Ledger(ledger))?)?;
    assert!(matches!(
        decode_store_state(text.replace("\"schema_version\":1", "\"schema_version\":0").as_bytes()),
        Err(IncidentStoreError::UnsupportedSchema(0))
    ));
    let text = String::from_utf8(encode_symbol_report(&SymbolOutcome::NotRequested)?)?;
    assert!(matches!(
        decode_symbol_report(
            text.replace("\"schema_version\":1", "\"schema_version\":2").as_bytes()
        ),
        Err(IncidentStoreError::UnsupportedSchema(2))
    ));
    let error = decode_summary(b"{}");
    assert!(error.as_ref().err().and_then(|cause| cause.source()).is_some());
    Ok(())
}
