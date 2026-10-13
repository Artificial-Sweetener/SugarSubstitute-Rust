// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify strict forward conversion and independent immutable attachment generations.
use super::*;

fn retained_summary() -> Result<IncidentSummary, Box<dyn Error>> {
    let baseline = summary()?;
    let facts = baseline.facts();
    Ok(IncidentSummary::new(
        baseline.seed().clone(),
        baseline.revision(),
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            PayloadDisposition::Retained(AttachmentInventory::new(&[AttachmentFacts::new(
                AttachmentKind::Log,
                4,
                4,
                0,
                [9; 32],
            )?])?),
            facts.reporting(),
            facts.symbols().clone(),
        ),
    )?)
}
fn legacy(summary: &IncidentSummary) -> Result<Vec<u8>, IncidentStoreError> {
    super::super::codec_encode::encode(
        1,
        super::super::codec_summary::SummaryRecord::from(summary),
        65_536,
    )
}
#[test]
fn schema_two_roundtrips_retained_and_unavailable_generations() -> Result<(), Box<dyn Error>> {
    let retained = retained_summary()?;
    let stored = StoredIncidentSummary::new(retained.clone(), Some(retained.revision()))?;
    let bytes = encode_summary(&stored)?;
    assert_eq!(decode_summary(&bytes)?, stored);
    assert!(std::str::from_utf8(&bytes)?.starts_with("{\"schema_version\":2,"));
    let unavailable = StoredIncidentSummary::new(summary()?, None)?;
    let bytes = encode_summary(&unavailable)?;
    assert!(std::str::from_utf8(&bytes)?.contains("\"payload_revision\":null"));
    assert_eq!(decode_summary(&bytes)?, unavailable);
    Ok(())
}
#[test]
fn payload_reference_is_required_nullable_bounded_and_consistent() -> Result<(), Box<dyn Error>> {
    let retained = retained_summary()?;
    assert!(StoredIncidentSummary::new(retained.clone(), None).is_err());
    assert!(StoredIncidentSummary::new(retained.clone(), Some(IncidentRevision::new(3)?)).is_err());
    assert!(StoredIncidentSummary::new(summary()?, Some(IncidentRevision::new(1)?)).is_err());
    let text = String::from_utf8(encode_summary(&StoredIncidentSummary::new(
        retained,
        Some(IncidentRevision::new(2)?),
    )?)?)?;
    for changed in [
        text.replace(",\"payload_revision\":2", ""),
        text.replace("\"payload_revision\":2", "\"payload_revision\":null"),
        text.replace("\"payload_revision\":2", "\"payload_revision\":0"),
        text.replace("\"payload_revision\":2", "\"payload_revision\":3"),
        text.replace("\"payload_revision\":2", "\"payload_revision\":2,\"payload_revision\":2"),
    ] {
        assert!(decode_summary(changed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn legacy_conversion_preserves_input_and_derives_only_legacy_revision() -> Result<(), Box<dyn Error>>
{
    let retained = retained_summary()?;
    let bytes = legacy(&retained)?;
    let original = bytes.clone();
    assert!(decode_summary(&bytes).is_err());
    let migrated = migrate_summary_v1(&bytes)?;
    let stored = decode_summary(&migrated)?;
    assert_eq!(stored.summary(), &retained);
    assert_eq!(stored.payload_revision(), Some(retained.revision()));
    assert_eq!(bytes, original);
    assert_eq!(migrate_summary_v1(&migrated)?, migrated);
    let unavailable = summary()?;
    assert_eq!(
        decode_summary(&migrate_summary_v1(&legacy(&unavailable)?)?)?.payload_revision(),
        None
    );
    Ok(())
}
#[test]
fn migration_rejects_unknown_fields_versions_duplicates_and_bounds() -> Result<(), Box<dyn Error>> {
    let text = String::from_utf8(legacy(&retained_summary()?)?)?;
    for changed in [
        text.replace("\"schema_version\":1", "\"schema_version\":1,\"private\":0"),
        text.replace("\"schema_version\":1", "\"schema_version\":3"),
        text.replace("\"revision\":2", "\"revision\":2,\"revision\":2"),
        text.replace("\"occurred\":101", "\"occurred\":101,\"unknown\":0"),
    ] {
        assert!(migrate_summary_v1(changed.as_bytes()).is_err());
    }
    let bytes = vec![b' '; 65_537];
    assert!(matches!(migrate_summary_v1(&bytes), Err(IncidentStoreError::RecordBound)));
    let valid = String::from_utf8(encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?)?;
    for changed in [
        valid.replace("\"summary\":{", "\"unknown\":0,\"summary\":{"),
        valid.replace("\"payload_revision\":null", "\"payload_revision\":null,\"unknown\":0"),
        valid.replace("\"schema_version\":2", "\"schema_version\":9"),
    ] {
        assert!(decode_summary(changed.as_bytes()).is_err());
        assert!(migrate_summary_v1(changed.as_bytes()).is_err());
    }
    Ok(())
}
#[test]
fn presented_and_acknowledged_revisions_preserve_original_payload_generation()
-> Result<(), Box<dyn Error>> {
    let baseline = retained_summary()?;
    let facts = baseline.facts();
    let presented = IncidentSummary::new(
        baseline.seed().clone(),
        IncidentRevision::new(3)?,
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
            facts.symbols().clone(),
        ),
    )?;
    let acknowledged = IncidentSummary::new(
        baseline.seed().clone(),
        IncidentRevision::new(4)?,
        SummaryFacts::new(
            RetentionFacts::new(
                facts.retention().occurred(),
                Some(IncidentUtc::from_unix_seconds(102)?),
            )?,
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            presented.facts().reporting(),
            facts.symbols().clone(),
        ),
    )?;
    for summary in [presented, acknowledged] {
        let stored = StoredIncidentSummary::new(summary, Some(baseline.revision()))?;
        let actual = decode_summary(&encode_summary(&stored)?)?;
        assert_eq!(actual.payload_revision(), Some(IncidentRevision::new(2)?));
        assert_eq!(actual.summary().facts().payload(), baseline.facts().payload());
        assert_eq!(actual.summary().facts().retention().occurred(), facts.retention().occurred());
    }
    Ok(())
}
#[test]
fn payload_generation_cannot_precede_historical_seed_revision() -> Result<(), Box<dyn Error>> {
    let baseline = retained_summary()?;
    let seed = IncidentSeed::new(
        baseline.seed().key(),
        IncidentRevision::new(4)?,
        baseline.seed().build().clone(),
        baseline.seed().created(),
        baseline.seed().stage(),
    )?;
    let summary = IncidentSummary::new(seed, IncidentRevision::new(5)?, baseline.facts().clone())?;
    assert!(StoredIncidentSummary::new(summary.clone(), Some(IncidentRevision::new(3)?)).is_err());
    assert!(StoredIncidentSummary::new(summary, Some(IncidentRevision::new(4)?)).is_ok());
    Ok(())
}
