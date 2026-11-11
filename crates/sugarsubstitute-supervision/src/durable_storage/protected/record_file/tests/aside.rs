// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve unreadable fixed records beside their target through real protected files.
use super::*;
use crate::{
    coordinator_entry::fixture::Harness,
    durable_storage::protected::{
        account,
        exclusion::ExclusionOpen,
        native_name::{NativeName, NativeSid},
        native_phase::NativePhase,
        profile_fixture,
    },
    native_claims::{Record, files::Budget},
};
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};

/// Unreadable shapes a crash, a foreign writer or disk damage can leave behind.
#[derive(Clone, Copy, Debug)]
enum Damage {
    Garbage,
    WrongNamespace,
    Truncated,
}

fn current_sid(budget: &Budget) -> Result<Vec<u8>, ClaimError> {
    let mut handles = Vec::with_capacity(64);
    let mut phase = NativePhase::ordinary(budget)?;
    let result = account::resolve_current(
        &mut handles,
        budget.clone(),
        account::PrimaryAccountAccess::Query,
        &mut phase,
    )
    .and_then(|current| Ok(current.sid.bytes()?.to_vec()));
    let mut cleanup = Ok(());
    for handle in handles.iter_mut().rev() {
        cleanup = ClaimError::finish(cleanup, handle.close());
    }
    ClaimError::finish(result, cleanup)
}

fn coordinator(namespace: [u8; 32]) -> Result<[u8; 128], ClaimError> {
    Ok(Record::coordinator(CoordinatorKey {
        namespace: NamespaceId::new(namespace),
        epoch: CoordinatorEpoch::new([2; 16])
            .map_err(|cause| ClaimError::new("test_epoch", cause))?,
    })
    .encode())
}

/// Publish a valid record, damage it in place, then move it aside under its held exclusion.
fn damage_and_set_aside(
    directory: &Path,
    damage: Damage,
    context: &JobContext,
) -> Result<(), ClaimError> {
    let memory = Arc::new(
        context.memory().claim(64 * 1024).map_err(|cause| ClaimError::new("test_memory", cause))?,
    );
    let budget = Budget::Protected(memory);
    let record = directory.join("namespace.record");
    let mut held = None;
    HeldFile::<128>::acquire_into(
        ExclusionOpen::CreateOrOpen,
        &mut held,
        NativeName::Ordinary(directory.join("namespace.claim")),
        NativeName::Ordinary(record.clone()),
        NativeSid::Ordinary(current_sid(&budget)?),
        budget.clone(),
        &mut NativePhase::ordinary(&budget)?,
    )?;
    let file = held.as_mut().ok_or_else(|| ClaimError::invalid("test_exclusion"))?;
    publish(file, &coordinator([1; 32])?, PublicationAuthority::Ordinary, context, |_| Ok(()))?;
    let io = |cause| ClaimError::new("test_damage", cause);
    let damaged = match damage {
        Damage::Garbage => vec![0x5a; 128],
        Damage::WrongNamespace => coordinator([7; 32])?.to_vec(),
        Damage::Truncated => coordinator([1; 32])?[..60].to_vec(),
    };
    {
        use std::io::Write;
        let mut writer = std::fs::OpenOptions::new().write(true).open(&record).map_err(io)?;
        writer.set_len(0).map_err(io)?;
        writer.write_all(&damaged).map_err(io)?;
    }
    match (damage, read_shape(file, context)) {
        (Damage::Truncated, Err(cause)) => assert!(is_unreadable_length(&cause), "{cause:?}"),
        (Damage::Garbage | Damage::WrongNamespace, Ok(RecordRead::Present(bytes))) => {
            assert_eq!(bytes.as_slice(), damaged.as_slice());
        }
        (_, Ok(_)) => return Err(ClaimError::invalid("test_unexpected_read")),
        (_, Err(cause)) => return Err(cause),
    }
    let aside = set_aside(file, context)?;
    assert_eq!(aside.parent(), record.parent());
    let leaf =
        aside.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    assert!(leaf.starts_with("namespace.record.") && leaf.ends_with(".unreadable"), "{leaf}");
    assert_eq!(std::fs::read(&aside).map_err(io)?, damaged);
    assert!(!record.exists());
    assert!(matches!(read_shape(file, context)?, RecordRead::Missing));
    assert!(matches!(
        set_aside(file, context),
        Err(cause) if cause.operation() == "record_aside_missing"
    ));
    file.close()?;
    drop(held);
    let mut unlocked = None;
    HeldFile::<128>::acquire_into(
        ExclusionOpen::CreateOrOpen,
        &mut unlocked,
        NativeName::Ordinary(directory.join("namespace.claim")),
        NativeName::Ordinary(record),
        NativeSid::Ordinary(current_sid(&budget)?),
        budget.clone(),
        &mut NativePhase::ordinary(&budget)?,
    )?;
    let file = unlocked.as_mut().ok_or_else(|| ClaimError::invalid("test_exclusion"))?;
    file.close()?;
    assert!(matches!(
        set_aside(file, context),
        Err(cause) if cause.operation() == "record_aside_without_exclusion"
    ));
    Ok(())
}

#[test]
fn unreadable_records_move_aside_with_their_exact_bytes_under_a_held_exclusion() -> TestResult {
    let mut harness = Harness::new()?;
    let mut spec = harness.spec(Moment::from_nanos(30_000_000_000))?;
    spec.reservation_bytes = crate::profiles::PROFILE_OPERATION_BYTES;
    let base = tempfile::tempdir()?;
    let parent = base.path().to_owned();
    let mut job = harness.client.submit_blocking(&harness.scope, spec, move |context| {
        for (index, damage) in
            [Damage::Garbage, Damage::WrongNamespace, Damage::Truncated].into_iter().enumerate()
        {
            let directory =
                profile_fixture::create(&parent.join(format!("namespace-{index}")), &context)?;
            damage_and_set_aside(&directory, damage, &context)
                .map_err(|cause| ClaimError::new("test_damage_case", cause))?;
        }
        Ok::<_, ClaimError>(())
    })?;
    let completion = harness.consume_job(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
    drop((completion, job));
    harness.close(Moment::from_nanos(30_000_000_000))?;
    harness.join_execution_after_retirement()?;
    drop(base);
    Ok(())
}
