// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify closed migration/archive data while preserving exact existing record grammar.
use super::*;

pub(super) fn journal() -> Result<SummaryMigrationJournal, Box<dyn Error>> {
    let summary = summary()?;
    let original = super::super::codec_encode::encode(
        1,
        super::super::codec_summary::SummaryRecord::from(&summary),
        65_536,
    )?;
    let candidate = migrate_summary_v1(&original)?;
    let original_digest = super::super::codec_ledger::digest(&original);
    Ok(SummaryMigrationJournal::new(
        LogicalStoreOperationKey::new(owner(2)?, IncidentOperationId::new(11)?),
        summary.seed().key(),
        summary.revision(),
        None,
        IncidentFileFacts::new([1; 24], original_digest, original.len() as u64)?,
        IncidentFileFacts::new([2; 24], original_digest, original.len() as u64)?,
        IncidentFileFacts::new(
            [3; 24],
            super::super::codec_ledger::digest(&candidate),
            candidate.len() as u64,
        )?,
    )?)
}
fn journal_text() -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(encode_store_state(&IncidentStoreState::SummaryMigration(Box::new(
        journal()?,
    )))?)?)
}
#[test]
fn migration_archive_roundtrip_preserves_full_historical_key_and_stage_roles()
-> Result<(), Box<dyn Error>> {
    let journal = journal()?;
    let state = IncidentStoreState::SummaryMigration(Box::new(journal.clone()));
    let bytes = encode_store_state(&state)?;
    assert_eq!(decode_store_state(&bytes)?, state);
    let text = std::str::from_utf8(&bytes)?;
    assert!(text.starts_with(
        "{\"schema_version\":1,\"content\":{\"record\":\"summary_migration\",\"operation\":"
    ));
    assert!(text.contains("\"source_schema\":1,\"candidate_schema\":2"));
    let archive = SummaryMigrationArchive::new(journal.clone())?;
    assert_eq!(archive.migration(), &journal);
    let state = IncidentStoreState::SummaryMigrationArchive(Box::new(archive));
    let bytes = encode_store_state(&state)?;
    assert_eq!(decode_store_state(&bytes)?, state);
    assert!(
        std::str::from_utf8(&bytes)?
            .contains("\"record\":\"summary_migration_archive\",\"migration\":{")
    );
    assert_eq!(journal.operation().owner(), owner(2)?);
    assert_eq!(journal.operation().id().get(), 11);
    assert_eq!(journal.original_stage().sha256(), journal.original_file().sha256());
    assert_ne!(journal.candidate_stage().identity(), journal.original_stage().identity());
    Ok(())
}
#[test]
fn migration_requires_fixed_forward_schemas_and_explicit_nullable_reference()
-> Result<(), Box<dyn Error>> {
    let text = journal_text()?;
    for changed in [
        text.replace("\"source_schema\":1", "\"source_schema\":0"),
        text.replace("\"source_schema\":1", "\"source_schema\":2"),
        text.replace("\"candidate_schema\":2", "\"candidate_schema\":1"),
        text.replace("\"candidate_schema\":2", "\"candidate_schema\":3"),
        text.replace("\"payload_revision\":null,", ""),
        text.replace("\"payload_revision\":null", "\"payload_revision\":0"),
        text.replace("\"payload_revision\":null", "\"payload_revision\":3"),
    ] {
        assert!(decode_store_state(changed.as_bytes()).is_err());
    }
    let accepted = text.replace("\"payload_revision\":null", "\"payload_revision\":2");
    let IncidentStoreState::SummaryMigration(journal) = decode_store_state(accepted.as_bytes())?
    else {
        return Err("Wrong migration variant".into());
    };
    assert_eq!(journal.payload_revision(), Some(IncidentRevision::new(2)?));
    Ok(())
}
#[test]
fn migration_original_copy_and_candidate_identities_are_complete_and_distinct()
-> Result<(), Box<dyn Error>> {
    let good = journal()?;
    let make = |original, copy, candidate| {
        SummaryMigrationJournal::new(
            good.operation(),
            good.incident(),
            good.revision(),
            None,
            original,
            copy,
            candidate,
        )
    };
    assert!(make(good.original_file(), good.original_file(), good.candidate_stage()).is_err());
    assert!(make(good.original_file(), good.original_stage(), good.original_file()).is_err());
    assert!(make(good.original_file(), good.original_stage(), good.original_stage()).is_err());
    let changed_hash = IncidentFileFacts::new([2; 24], [6; 32], good.original_stage().bytes())?;
    assert!(make(good.original_file(), changed_hash, good.candidate_stage()).is_err());
    let changed_length = IncidentFileFacts::new(
        [2; 24],
        good.original_stage().sha256(),
        good.original_stage().bytes() + 1,
    )?;
    assert!(make(good.original_file(), changed_length, good.candidate_stage()).is_err());
    let same_bytes = IncidentFileFacts::new(
        [3; 24],
        good.original_stage().sha256(),
        good.original_stage().bytes(),
    )?;
    assert!(make(good.original_file(), good.original_stage(), same_bytes).is_err());
    assert!(IncidentFileFacts::new([0; 24], [1; 32], 1).is_err());
    assert!(IncidentFileFacts::new([1; 24], [0; 32], 1).is_err());
    assert!(IncidentFileFacts::new([1; 24], [1; 32], 0).is_err());
    assert!(IncidentFileFacts::new([1; 24], [1; 32], 65_537).is_err());
    Ok(())
}
#[test]
fn migration_and_archive_reject_unknown_duplicate_fields_at_every_level()
-> Result<(), Box<dyn Error>> {
    let text = journal_text()?;
    for changed in [
        text.replace("\"schema_version\":1", "\"schema_version\":1,\"external\":0"),
        text.replace(
            "\"record\":\"summary_migration\"",
            "\"record\":\"summary_migration\",\"external\":0",
        ),
        text.replace("\"id\":11", "\"id\":11,\"external\":0"),
        text.replace("\"revision\":2", "\"revision\":2,\"revision\":2"),
        text.replace("\"bytes\":", "\"external\":0,\"bytes\":"),
        text.replace(
            "\"payload_revision\":null",
            "\"payload_revision\":null,\"payload_revision\":null",
        ),
    ] {
        assert!(decode_store_state(changed.as_bytes()).is_err());
    }
    let archive = IncidentStoreState::SummaryMigrationArchive(Box::new(
        SummaryMigrationArchive::new(journal()?)?,
    ));
    let text = String::from_utf8(encode_store_state(&archive)?)?;
    for changed in [
        text.replace("\"migration\":{", "\"external\":0,\"migration\":{"),
        text.replace("\"source_schema\":1", "\"source_schema\":1,\"external\":0"),
        text.replace("\"candidate_schema\":2", "\"candidate_schema\":2,\"candidate_schema\":2"),
    ] {
        assert!(decode_store_state(changed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn migration_namespace_and_historical_coordinator_cannot_be_inferred_or_replaced()
-> Result<(), Box<dyn Error>> {
    let good = journal()?;
    let foreign = CoordinatorKey {
        namespace: NamespaceId::new([9; 32]),
        epoch: CoordinatorEpoch::new([8; 16])?,
    };
    assert!(
        SummaryMigrationJournal::new(
            LogicalStoreOperationKey::new(foreign, IncidentOperationId::new(11)?),
            good.incident(),
            good.revision(),
            None,
            good.original_file(),
            good.original_stage(),
            good.candidate_stage()
        )
        .is_err()
    );
    let next = SummaryMigrationJournal::new(
        LogicalStoreOperationKey::new(owner(3)?, IncidentOperationId::new(11)?),
        good.incident(),
        good.revision(),
        None,
        good.original_file(),
        good.original_stage(),
        good.candidate_stage(),
    )?;
    assert_ne!(good.operation(), next.operation());
    assert_eq!(good.incident(), next.incident());
    let state = IncidentStoreState::SummaryMigration(Box::new(next.clone()));
    assert_eq!(decode_store_state(&encode_store_state(&state)?)?, state);
    assert_eq!(next.operation().owner(), owner(3)?);
    Ok(())
}
#[test]
fn migration_metadata_bounds_versions_and_unknown_variants_preserve_rejection()
-> Result<(), Box<dyn Error>> {
    let text = journal_text()?;
    assert!(matches!(
        decode_store_state(text.replace("\"schema_version\":1", "\"schema_version\":2").as_bytes()),
        Err(IncidentStoreError::UnsupportedSchema(2))
    ));
    assert!(
        decode_store_state(
            text.replace("summary_migration", "summary_migration_unknown").as_bytes()
        )
        .is_err()
    );
    assert!(decode_store_state(format!("{text} {{}}").as_bytes()).is_err());
    assert!(matches!(
        decode_store_state(&vec![b' '; 65_537]),
        Err(IncidentStoreError::RecordBound)
    ));
    let state = IncidentStoreState::SummaryMigrationArchive(Box::new(
        SummaryMigrationArchive::new(journal()?)?,
    ));
    let original = encode_store_state(&state)?;
    let decoded = decode_store_state(&original)?;
    assert_eq!(encode_store_state(&decoded)?, original);
    Ok(())
}
#[test]
fn archive_data_remains_historical_when_live_summary_revision_advances()
-> Result<(), Box<dyn Error>> {
    let journal = journal()?;
    let historical = journal.revision();
    let archive = IncidentStoreState::SummaryMigrationArchive(Box::new(
        SummaryMigrationArchive::new(journal)?,
    ));
    let original = encode_store_state(&archive)?;
    let baseline = summary()?;
    let advanced = IncidentSummary::new(
        baseline.seed().clone(),
        baseline.revision().next()?,
        baseline.facts().clone(),
    )?;
    let stored = StoredIncidentSummary::new(advanced, None)?;
    assert_eq!(decode_summary(&encode_summary(&stored)?)?.summary().revision(), historical.next()?);
    assert_eq!(encode_store_state(&decode_store_state(&original)?)?, original);
    Ok(())
}
#[test]
fn existing_ledger_encoding_remains_exact_schema_one_bytes() -> Result<(), Box<dyn Error>> {
    let ledger = SummaryEvictions::new(owner(2)?.namespace, 0, 0, 0, None)?;
    let expected = format!(
        "{{\"schema_version\":1,\"content\":{{\"record\":\"ledger\",\"ledger\":{{\"namespace\":[{}],\"pending\":0,\"acknowledged\":0,\"quarantined\":0,\"last\":null}}}}}}",
        ["1"; 32].join(",")
    );
    let encoded = encode_store_state(&IncidentStoreState::Ledger(ledger))?;
    assert_eq!(std::str::from_utf8(&encoded)?, expected);
    assert_eq!(decode_store_state(&encoded)?, IncidentStoreState::Ledger(ledger));
    Ok(())
}

#[test]
fn migration_decoder_redacts_persisted_fields_and_preserves_original_cause()
-> Result<(), Box<dyn Error>> {
    let sentinel = "PRIVATE_MIGRATION_FIELD_d92e6";
    let journal = journal_text()?;
    let changed =
        journal.replace("\"source_schema\":1", &format!("\"source_schema\":1,\"{sentinel}\":0"));
    let error =
        decode_store_state(changed.as_bytes()).err().ok_or("Unknown migration field accepted")?;
    assert!(!error.to_string().contains(sentinel));
    assert!(!format!("{error:?}").contains(sentinel));
    let source = error.source().ok_or("Original decoder cause lost")?;
    assert!(source.downcast_ref::<serde_json::Error>().is_some());
    assert!(source.to_string().contains("Unknown incident field"));
    assert!(!source.to_string().contains(sentinel));
    let json = source.downcast_ref::<serde_json::Error>().ok_or("JSON cause lost")?;
    assert!(json.line() > 0 && json.column() > 0);
    Ok(())
}
