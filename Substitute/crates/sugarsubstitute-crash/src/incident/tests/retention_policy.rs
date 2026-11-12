// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify deterministic bounded selection independently of native disposition authority.
use super::super::retention_policy::select_payload;
use super::*;
fn retained(instance: u64, ack: Option<i64>) -> Result<StoredIncidentSummary, Box<dyn Error>> {
    let base = summary()?;
    let facts = base.facts();
    let key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner: owner(2)?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(4)?,
    }));
    let seed = IncidentSeed::new(
        key,
        base.seed().revision(),
        base.seed().build().clone(),
        base.seed().created(),
        base.seed().stage(),
    )?;
    Ok(StoredIncidentSummary::new(
        IncidentSummary::new(
            seed,
            base.revision(),
            SummaryFacts::new(
                RetentionFacts::new(
                    facts.retention().occurred(),
                    ack.map(IncidentUtc::from_unix_seconds).transpose()?,
                )?,
                facts.exit(),
                facts.failure(),
                facts.capture(),
                PayloadDisposition::Retained(AttachmentInventory::new(&[AttachmentFacts::new(
                    AttachmentKind::Fault,
                    7,
                    7,
                    0,
                    [2; 32],
                )?])?),
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?,
        Some(base.revision()),
    )?)
}
#[test]
fn retention_age_and_acknowledged_count_use_exact_edges() -> Result<(), Box<dyn Error>> {
    let old = retained(1, Some(110))?;
    let edge = 110 + ACKNOWLEDGED_RETENTION_SECONDS as i64;
    assert!(
        select_payload(
            std::slice::from_ref(&old).iter(),
            IncidentUtc::from_unix_seconds(edge - 1)?,
            7,
            1,
            0,
            0,
            None
        )?
        .is_none()
    );
    let selected =
        select_payload([old].iter(), IncidentUtc::from_unix_seconds(edge)?, 7, 1, 0, 0, None)?
            .ok_or("90day edge omitted")?;
    assert_eq!(selected.reason, PayloadDiscard::AcknowledgedRetention);
    let mut entries =
        (1..=20).map(|instance| retained(instance, Some(110))).collect::<Result<Vec<_>, _>>()?;
    let now = IncidentUtc::from_unix_seconds(111)?;
    assert!(select_payload(entries.iter(), now, 140, 20, 0, 0, None)?.is_none());
    entries.push(retained(21, Some(110))?);
    assert_eq!(
        select_payload(entries.iter(), now, 147, 21, 0, 0, None)?.ok_or("21ack edge omitted")?.key,
        entries[0].summary().seed().key()
    );
    Ok(())
}
#[test]
fn retention_projection_prefers_ack_then_oldest_pending_with_full_key_ties()
-> Result<(), Box<dyn Error>> {
    let pending = retained(1, None)?;
    let ack = retained(2, Some(110))?;
    let now = IncidentUtc::from_unix_seconds(111)?;
    let selected = select_payload(
        [pending.clone(), ack.clone()].iter(),
        now,
        MAX_PAYLOAD_BYTES,
        2,
        1,
        0,
        None,
    )?
    .ok_or("byte pressure omitted")?;
    assert_eq!(selected.key, ack.summary().seed().key());
    assert_eq!(selected.reason, PayloadDiscard::StoragePressure);
    assert!(
        select_payload(std::slice::from_ref(&pending).iter(), now, 7, 32, 0, 0, None)?.is_none()
    );
    assert!(
        select_payload(std::slice::from_ref(&pending).iter(), now, 7, 32, 0, 1, None)?.is_some()
    );
    let later_key = retained(9, None)?;
    for ordered in [vec![pending.clone(), later_key.clone()], vec![later_key, pending.clone()]] {
        assert_eq!(
            select_payload(ordered.iter(), now, MAX_PAYLOAD_BYTES, 2, 1, 0, None)?
                .ok_or("tie absent")?
                .key,
            pending.summary().seed().key()
        );
    }
    Ok(())
}
#[test]
fn retention_rejects_utc_regression_duplicate_keys_overflow_and_over_cap_totals()
-> Result<(), Box<dyn Error>> {
    let old = retained(1, Some(110))?;
    let now = IncidentUtc::from_unix_seconds(111)?;
    assert!(
        select_payload(
            std::slice::from_ref(&old).iter(),
            IncidentUtc::from_unix_seconds(109)?,
            7,
            1,
            0,
            0,
            None
        )
        .is_err()
    );
    assert!(select_payload([old.clone(), old.clone()].iter(), now, 14, 2, 0, 0, None).is_err());
    assert!(
        select_payload(std::slice::from_ref(&old).iter(), now, 7, 1, u64::MAX, 0, None).is_err()
    );
    assert!(
        select_payload(std::slice::from_ref(&old).iter(), now, 7, 1, 0, usize::MAX, None).is_err()
    );
    assert!(
        select_payload(
            std::slice::from_ref(&old).iter(),
            now,
            MAX_PAYLOAD_BYTES + 1,
            1,
            0,
            0,
            None
        )
        .is_err()
    );
    assert!(select_payload(std::slice::from_ref(&old).iter(), now, 7, 33, 0, 0, None).is_err());
    assert!(select_payload(vec![old; 129].iter(), now, 7, 1, 0, 0, None).is_err());
    Ok(())
}

#[test]
fn retention_ties_keep_historical_epoch_and_generation_in_the_full_key()
-> Result<(), Box<dyn Error>> {
    let base = retained(9, None)?;
    let make = |epoch, instance, generation| -> Result<StoredIncidentSummary, Box<dyn Error>> {
        let summary = base.summary();
        let seed = summary.seed();
        let key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner: owner(epoch)?,
            instance: InstanceId::new(instance)?,
            generation: RunGeneration::new(generation)?,
        }));
        let changed = IncidentSeed::new(
            key,
            seed.revision(),
            seed.build().clone(),
            seed.created(),
            seed.stage(),
        )?;
        Ok(StoredIncidentSummary::new(
            IncidentSummary::new(changed, summary.revision(), summary.facts().clone())?,
            base.payload_revision(),
        )?)
    };
    let oldest = make(1, 9, 2)?;
    let next_generation = make(1, 9, 4)?;
    let next_epoch = make(9, 1, 1)?;
    let selected = select_payload(
        [next_epoch, next_generation, oldest.clone()].iter(),
        IncidentUtc::from_unix_seconds(111)?,
        MAX_PAYLOAD_BYTES,
        3,
        1,
        0,
        None,
    )?
    .ok_or("full-key tie absent")?;
    assert_eq!(selected.key, oldest.summary().seed().key());
    Ok(())
}

#[test]
fn excluded_incoming_summary_still_counts_toward_global_acknowledged_limit()
-> Result<(), Box<dyn Error>> {
    let records =
        (1..=21).map(|instance| retained(instance, Some(110))).collect::<Result<Vec<_>, _>>()?;
    let excluded = records[0].summary().seed().key();
    let choice = select_payload(
        records.iter(),
        IncidentUtc::from_unix_seconds(111)?,
        147,
        21,
        0,
        0,
        Some(excluded),
    )?
    .ok_or("protected record hid global excess")?;
    assert_eq!(choice.key, records[1].summary().seed().key());
    assert_eq!(choice.reason, PayloadDiscard::AcknowledgedRetention);
    Ok(())
}
