// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise version transitions, restart boundaries and hostile durable input without native fakes.
use super::*;
mod fallback;
mod strikes;
mod supersession;
/// UTC Unix seconds the transitions under test observe, unless a test moves time itself.
const NOW: u64 = 1_800_000_000;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn empty() -> Result<InstallationRecord> {
    Ok(InstallationRecord::empty(InstallationId::new("1".repeat(32))?))
}
fn generation(number: u64) -> Result<Generation> {
    Ok(Generation {
        id: GenerationId::new(format!("{number:032x}"))?,
        package: PackageDigest::new(format!("{number:064x}"))?,
        release: crate::installation::SignedRelease::new(format!("1.0.{number}"), number + 1)?,
    })
}
fn reload(record: &InstallationRecord) -> Result<InstallationRecord> {
    Ok(InstallationRecord::decode(&record.encode()?)?)
}
fn attempt(record: &InstallationRecord) -> Result<Trial> {
    match record.candidate() {
        Some(Candidate::Trial { attempt }) => Ok(attempt.clone()),
        _ => Err("Expected candidate trial".into()),
    }
}
fn prepare(record: &InstallationRecord, generation: &Generation) -> Result<InstallationRecord> {
    let reserved =
        reload(&record.prepare(generation.clone(), PreparationIntent::Automatic, NOW)?)?;
    reload(&reserved.prepared(generation)?)
}
/// The strike `count` bad verdicts of `generation` leave, the latest at `at`.
fn struck(generation: &Generation, count: u8, at: u64) -> Strike {
    Strike {
        package: generation.package.clone(),
        sequence: generation.release.sequence(),
        count,
        at,
    }
}
/// Judge one trial of `generation` bad at `at`, starting from `record`.
fn strike(
    record: &InstallationRecord,
    generation: &Generation,
    at: u64,
) -> Result<InstallationRecord> {
    let trial = reload(
        &record
            .prepare(generation.clone(), PreparationIntent::Repair, at)?
            .prepared(generation)?
            .begin_trial()?,
    )?;
    reload(&trial.reject(&attempt(&trial)?, at)?)
}
fn install(record: &InstallationRecord, generation: &Generation) -> Result<InstallationRecord> {
    let trial = reload(&prepare(record, generation)?.begin_trial()?)?;
    reload(&trial.accept(&attempt(&trial)?)?)
}

#[test]
fn interrupted_preparation_preserves_accepted_launch_and_requires_no_process_guessing() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&empty()?, &a)?;
    let staging = reload(&accepted.prepare(b.clone(), PreparationIntent::Automatic, NOW)?)?;
    let prepared = reload(&staging.prepared(&b)?)?;
    for state in [&accepted, &staging, &prepared] {
        assert_eq!(state.startup_selection(), StartupSelection::Accepted(&a));
    }
    let trial = reload(&prepared.begin_trial()?)?;
    assert_eq!(trial.accepted(), Some(&a));
    assert_eq!(trial.startup_selection(), StartupSelection::ReconcileTrial(&attempt(&trial)?));
    assert!(trial.cancel_preparation(&b).is_err());
    Ok(())
}

#[test]
fn broken_b_recovers_a_and_c_can_be_accepted_after_reloading_every_transition() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let c = generation(3)?;
    let accepted = install(&empty()?, &a)?;
    let trial = reload(&prepare(&accepted, &b)?.begin_trial()?)?;
    let recovered = reload(&trial.reject(&attempt(&trial)?, NOW)?)?;
    assert_eq!(recovered.startup_selection(), StartupSelection::Accepted(&a));
    assert_eq!(recovered.rollback(), Some(&b));
    assert_eq!(recovered.strikes(), &[struck(&b, 1, NOW)]);
    let accepted_c = install(&recovered, &c)?;
    assert_eq!(accepted_c.accepted(), Some(&c));
    assert_eq!(accepted_c.retained(), Some(&a));
    assert!(accepted_c.candidate().is_none());
    assert_eq!(accepted_c.rollback(), None);
    assert_eq!(accepted_c.strikes(), &[struck(&b, 1, NOW)]);
    Ok(())
}

#[test]
fn unstarted_trial_returns_to_prepared_without_quarantine_or_losing_accepted_history() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&empty()?, &a)?;
    let trial = reload(&prepare(&accepted, &b)?.begin_trial()?)?;
    let original = attempt(&trial)?;
    let mut foreign = original.clone();
    foreign.revision += 1;
    assert_eq!(trial.cancel_unstarted_trial(&foreign), Err(Rejection::Binding));
    let prepared = reload(&trial.cancel_unstarted_trial(&original)?)?;
    assert_eq!(prepared.startup_selection(), StartupSelection::Accepted(&a));
    assert_eq!(prepared.candidate(), Some(&Candidate::Prepared { generation: b }));
    assert_eq!(prepared.retained(), accepted.retained());
    assert_eq!(prepared.strikes(), accepted.strikes());
    assert_eq!(prepared.revision(), trial.revision() + 1);
    assert!(prepared.cancel_unstarted_trial(&original).is_err());
    let retry = reload(&prepared.begin_trial()?)?;
    assert_ne!(attempt(&retry)?, original);
    assert!(retry.accept(&original).is_err());
    assert!(retry.cancel_unstarted_trial(&original).is_err());
    Ok(())
}

#[test]
fn late_cross_installation_or_forged_health_observations_cannot_accept_a_trial() -> Result {
    let candidate = generation(1)?;
    let trial = prepare(&empty()?, &candidate)?.begin_trial()?;
    let receipt = attempt(&trial)?;
    let before = trial.encode()?;
    let mut wrong = receipt.clone();
    wrong.installation = InstallationId::new("2".repeat(32))?;
    assert_eq!(trial.accept(&wrong), Err(Rejection::Binding));
    wrong = receipt.clone();
    wrong.generation.package = generation(2)?.package;
    assert_eq!(trial.accept(&wrong), Err(Rejection::Binding));
    wrong = receipt.clone();
    wrong.revision += 1;
    assert_eq!(trial.reject(&wrong, NOW), Err(Rejection::Binding));
    assert_eq!(trial.release_trial(&wrong), Err(Rejection::Binding));
    assert_eq!(trial.abandon_trial(&wrong, NOW), Err(Rejection::Binding));
    let accepted = trial.accept(&receipt)?;
    assert_eq!(accepted.accept(&receipt), Err(Rejection::Phase));
    assert_eq!(trial.encode()?, before);
    Ok(())
}

#[test]
fn explicit_repair_gets_a_new_attempt_and_success_clears_package_quarantine() -> Result {
    let candidate = generation(1)?;
    let first = prepare(&empty()?, &candidate)?.begin_trial()?;
    let old_receipt = attempt(&first)?;
    let struck = first.reject(&old_receipt, NOW)?;
    let rejected = struck
        .prepare(candidate.clone(), PreparationIntent::Automatic, NOW)?
        .prepared(&candidate)?
        .begin_trial()?;
    let rejected = rejected.reject(&attempt(&rejected)?, NOW)?;
    assert_eq!(
        rejected.prepare(candidate.clone(), PreparationIntent::Automatic, NOW),
        Err(Rejection::Quarantined)
    );
    let second = rejected
        .prepare(candidate.clone(), PreparationIntent::Repair, NOW)?
        .prepared(&candidate)?
        .begin_trial()?;
    assert_ne!(attempt(&second)?, old_receipt);
    assert_eq!(second.accept(&old_receipt), Err(Rejection::Binding));
    let accepted = reload(&second.accept(&attempt(&second)?)?)?;
    assert!(accepted.strikes().is_empty());
    assert!(accepted.rollback().is_none());
    assert_eq!(accepted.accepted(), Some(&candidate));
    Ok(())
}

#[test]
fn fresh_install_and_failed_first_trial_never_claim_an_application_is_ready() -> Result {
    let initial = empty()?;
    let candidate = generation(1)?;
    assert_eq!(initial.startup_selection(), StartupSelection::NeedsInstallation);
    let staging = initial.prepare(candidate.clone(), PreparationIntent::Automatic, NOW)?;
    assert_eq!(staging.begin_trial(), Err(Rejection::Phase));
    let trial = staging.prepared(&candidate)?.begin_trial()?;
    let failed = reload(&trial.reject(&attempt(&trial)?, NOW)?)?;
    assert_eq!(failed.startup_selection(), StartupSelection::NeedsInstallation);
    assert!(failed.accepted().is_none());
    Ok(())
}

#[test]
fn cancellation_does_not_quarantine_and_preparation_completions_are_exact() -> Result {
    let candidate = generation(1)?;
    let staging = empty()?.prepare(candidate.clone(), PreparationIntent::Automatic, NOW)?;
    assert_eq!(staging.prepared(&generation(2)?), Err(Rejection::Binding));
    assert_eq!(staging.cancel_preparation(&generation(2)?), Err(Rejection::Binding));
    assert_eq!(
        staging.prepare(generation(2)?, PreparationIntent::Automatic, NOW),
        Err(Rejection::Phase)
    );
    for phase in [staging.clone(), staging.prepared(&candidate)?] {
        let cancelled = reload(&phase.cancel_preparation(&candidate)?)?;
        assert!(cancelled.candidate().is_none());
        assert!(cancelled.strikes().is_empty());
        assert!(cancelled.prepare(candidate.clone(), PreparationIntent::Automatic, NOW).is_ok());
    }
    Ok(())
}

#[test]
fn generations_cannot_alias_accepted_or_retained_directories() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let record = install(&install(&empty()?, &a)?, &b)?;
    for old in [a, b] {
        let conflicting = Generation {
            id: old.id,
            release: generation(3)?.release,
            package: generation(3)?.package,
        };
        assert_eq!(
            record.prepare(conflicting, PreparationIntent::Repair, NOW),
            Err(Rejection::Invariant)
        );
    }
    Ok(())
}

#[test]
fn hostile_records_cannot_erase_selection_by_omitting_fields_or_exceed_capacity() -> Result {
    let record = install(&empty()?, &generation(1)?)?;
    let value: serde_json::Value = serde_json::from_slice(&record.encode()?)?;
    for field in [
        "schema_version",
        "installation",
        "revision",
        "accepted",
        "retained",
        "candidate",
        "strikes",
        "unwitnessed",
        "rollback",
        "repair",
    ] {
        let mut changed = value.clone();
        changed.as_object_mut().ok_or("Object absent")?.remove(field);
        assert!(
            InstallationRecord::decode(&serde_json::to_vec(&changed)?).is_err(),
            "Missing {field}"
        );
    }
    // A later release may add fields within this schema; this reader keeps what it knows.
    let mut changed = value.clone();
    changed["extra"] = true.into();
    changed["accepted"]["channel"] = "beta".into();
    assert_eq!(InstallationRecord::decode(&serde_json::to_vec(&changed)?)?, record);
    let mut unlabelled = value.clone();
    unlabelled["accepted"].as_object_mut().ok_or("Accepted absent")?.remove("release");
    assert!(InstallationRecord::decode(&serde_json::to_vec(&unlabelled)?).is_err());
    changed = value;
    changed["schema_version"] = 1.into();
    assert_eq!(InstallationRecord::decode(&serde_json::to_vec(&changed)?), Err(Rejection::Schema));
    assert_eq!(
        InstallationRecord::decode(&vec![b' '; MAX_RECORD_BYTES + 1]),
        Err(Rejection::Capacity)
    );
    Ok(())
}

#[test]
fn record_validation_rejects_duplicate_selection_and_mismatched_trial_identity() -> Result {
    let a = generation(1)?;
    let record = install(&empty()?, &a)?;
    let mut value: serde_json::Value = serde_json::from_slice(&record.encode()?)?;
    value["retained"] = value["accepted"].clone();
    assert_eq!(InstallationRecord::decode(&serde_json::to_vec(&value)?), Err(Rejection::Invariant));
    let trial = prepare(&record, &generation(2)?)?.begin_trial()?;
    value = serde_json::from_slice(&trial.encode()?)?;
    value["candidate"]["attempt"]["revision"] = 0.into();
    assert_eq!(InstallationRecord::decode(&serde_json::to_vec(&value)?), Err(Rejection::Binding));
    Ok(())
}

#[test]
fn revision_exhaustion_preserves_record_and_quarantine_is_bounded_and_unique() -> Result {
    let mut record = empty()?;
    for number in 1..=20 {
        let trial = prepare(&record, &generation(number)?)?.begin_trial()?;
        record = reload(&trial.reject(&attempt(&trial)?, NOW)?)?;
    }
    assert_eq!(record.strikes().len(), MAX_STRUCK_PACKAGES);
    assert_eq!(
        record.strikes().first().map(|strike| &strike.package),
        Some(&generation(5)?.package)
    );
    let mut value: serde_json::Value = serde_json::from_slice(&record.encode()?)?;
    value["revision"] = u64::MAX.into();
    let exhausted = InstallationRecord::decode(&serde_json::to_vec(&value)?)?;
    assert_eq!(
        exhausted.prepare(generation(21)?, PreparationIntent::Automatic, NOW),
        Err(Rejection::Capacity)
    );
    assert_eq!(exhausted.revision(), u64::MAX);
    Ok(())
}

#[test]
fn identifiers_reject_paths_case_aliases_and_ambiguous_lengths() -> Result {
    for text in [
        "",
        "../generation",
        "A234567890abcdef1234567890abcdef",
        "1/234567890abcdef1234567890abcdef",
        "abcd",
    ] {
        assert!(GenerationId::new(text.into()).is_err());
        assert!(InstallationId::new(text.into()).is_err());
    }
    assert!(PackageDigest::new("A".repeat(64)).is_err());
    let encoded = serde_json::to_vec(&GenerationId::new("a".repeat(32))?)?;
    assert_eq!(
        serde_json::from_slice::<GenerationId>(&encoded)?,
        GenerationId::new("a".repeat(32))?
    );
    assert!(serde_json::from_str::<GenerationId>("\"../outside\"").is_err());
    Ok(())
}

#[test]
fn signed_release_labels_admit_only_what_a_signed_offer_carries() -> Result {
    let label = SignedRelease::new("1.2.3-beta.4+build".to_owned(), 7)?;
    assert_eq!((label.version(), label.sequence()), ("1.2.3-beta.4+build", 7));
    for (version, sequence) in [
        (String::new(), 1),
        ("1.0.0".to_owned(), 0),
        ("1.0 0".to_owned(), 1),
        ("../1".to_owned(), 1),
        ("1".repeat(SignedRelease::MAX_VERSION_BYTES + 1), 1),
    ] {
        assert_eq!(SignedRelease::new(version, sequence), Err(Rejection::Identity));
    }
    let record = install(&empty()?, &generation(1)?)?;
    let mut value: serde_json::Value = serde_json::from_slice(&record.encode()?)?;
    value["accepted"]["release"]["sequence"] = 0.into();
    assert!(InstallationRecord::decode(&serde_json::to_vec(&value)?).is_err());
    Ok(())
}

/// A record written by a release on either side of schema 3 is a typed `Schema` refusal, never
/// damaged bytes and never a record read with fields silently missing or ignored.
#[test]
fn records_from_older_and_newer_schemas_are_refused_as_unsupported_schemas() -> Result {
    let a = generation(1)?;
    let record = install(&empty()?, &a)?;
    let current: serde_json::Value = serde_json::from_slice(&record.encode()?)?;
    let mut older = current.clone();
    older["schema_version"] = 2.into();
    let older_fields = older.as_object_mut().ok_or("Object absent")?;
    older_fields.remove("strikes");
    older_fields.remove("unwitnessed");
    older_fields.insert("rejected".to_owned(), serde_json::json!([]));
    let mut newer = current;
    newer["schema_version"] = 4.into();
    newer["accepted"]["channel"] = "beta".into();
    newer["future"] = serde_json::json!({"strikes": 2});
    for skewed in [older, newer] {
        assert_eq!(
            InstallationRecord::decode(&serde_json::to_vec(&skewed)?),
            Err(Rejection::Schema)
        );
    }
    assert_eq!(InstallationRecord::decode(br#"{"revision":1}"#), Err(Rejection::Encoding));
    Ok(())
}
