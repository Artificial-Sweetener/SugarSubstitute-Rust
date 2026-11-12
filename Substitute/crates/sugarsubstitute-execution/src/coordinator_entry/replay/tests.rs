// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify replay policy independently of native proof acquisition and scheduling.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryConnectionId, EntryNativeIdentityFacts},
    execution::{CounterDeadline, MemoryRelease},
    lifecycle::{CoordinatorEpoch, NamespaceId},
};
struct Reservation;
impl MemoryRelease for Reservation {
    fn release(&self, _: u64) {}
}
fn cache() -> Result<Replay, EntryTransportError> {
    Replay::new(&MemoryLease::new(MIN_ENTRY_SERVER_BYTES, Box::new(Reservation)))
}
fn peer(pid: u32) -> Result<VerifiedEntryPeer, Box<dyn Error>> {
    Ok(VerifiedEntryPeer {
        connection: EntryConnectionId::new(1)?,
        facts: EntryNativeIdentityFacts::new(
            NamespaceId::new([1; 32]),
            &[1, 0, 0, 0, 0, 0, 0, 5],
            1,
            [4; 24],
        )?,
        pid,
        birth: 123,
    })
}
fn header(id: u64) -> Result<Header, Box<dyn Error>> {
    Ok(Header {
        namespace: NamespaceId::new([1; 32]),
        epoch: Some(CoordinatorEpoch::new([2; 16])?),
        request: EntryRequestId::new(id)?,
        source: CounterDeadline::new(i64::try_from(id)?, 10)?,
        nonce: [3; 32],
    })
}
fn consider(cache: &mut Replay, id: u64) -> Result<Decision, Box<dyn Error>> {
    Ok(cache.consider(
        peer(1)?,
        header(id)?,
        b"immutable body",
        Moment::from_nanos(1),
        Moment::from_nanos(100),
    )?)
}
#[test]
fn sixty_three_pending_reserve_one_cached_capacity_denial() -> Result<(), Box<dyn Error>> {
    let mut cache = cache()?;
    for id in 1..64 {
        assert!(matches!(consider(&mut cache, id)?, Decision::New(_)));
    }
    assert!(matches!(
        consider(&mut cache, 64)?,
        Decision::Cached { outcome: EntryOutcome::Rejected(EntryRejection::Capacity), .. }
    ));
    assert!(matches!(
        consider(&mut cache, 64)?,
        Decision::Cached { outcome: EntryOutcome::Rejected(EntryRejection::Capacity), .. }
    ));
    assert!(matches!(
        consider(&mut cache, 65)?,
        Decision::Cached { outcome: EntryOutcome::Rejected(EntryRejection::Capacity), .. }
    ));
    assert_eq!(cache.floor, 64);
    assert!(matches!(consider(&mut cache, 1)?, Decision::Pending));
    assert!(matches!(consider(&mut cache, 64)?, Decision::Rejected(EntryRejection::StaleRequest)));
    Ok(())
}
#[test]
fn only_lowest_settled_eviction_advances_floor_and_delayed_unseen_is_stale()
-> Result<(), Box<dyn Error>> {
    let mut cache = cache()?;
    for id in 10..74 {
        consider(&mut cache, id)?;
    }
    cache.settle(
        peer(1)?,
        EntryRequestId::new(15)?,
        EntryOutcome::Rejected(EntryRejection::ProfileBusy),
    )?;
    cache.settle(
        peer(1)?,
        EntryRequestId::new(20)?,
        EntryOutcome::Rejected(EntryRejection::InvalidInput),
    )?;
    assert!(matches!(consider(&mut cache, 74)?, Decision::New(_)));
    assert_eq!(cache.floor, 15);
    assert!(matches!(consider(&mut cache, 10)?, Decision::Pending));
    assert!(matches!(consider(&mut cache, 14)?, Decision::Pending));
    assert!(matches!(consider(&mut cache, 9)?, Decision::Rejected(EntryRejection::StaleRequest)));
    assert!(matches!(consider(&mut cache, 15)?, Decision::Rejected(EntryRejection::StaleRequest)));
    assert!(matches!(
        consider(&mut cache, 20)?,
        Decision::Cached { outcome: EntryOutcome::Rejected(EntryRejection::InvalidInput), .. }
    ));
    Ok(())
}
#[test]
fn reconnect_nonce_changes_preserve_semantics_and_original_cap() -> Result<(), Box<dyn Error>> {
    let mut cache = cache()?;
    let Decision::New(index) = consider(&mut cache, 50)? else {
        return Err("expected admission".into());
    };
    assert_eq!(cache.budget(index)?, (Moment::from_nanos(1), Moment::from_nanos(100)));
    let outcome = EntryOutcome::Rejected(EntryRejection::ProfileBusy);
    cache.settle(peer(1)?, EntryRequestId::new(50)?, outcome)?;
    let mut reconnect = header(50)?;
    reconnect.nonce = [9; 32];
    assert!(
        matches!(cache.consider(peer(1)?,reconnect,b"immutable body",Moment::from_nanos(2),Moment::from_nanos(200))?,Decision::Cached { until, .. } if until==Moment::from_nanos(100))
    );
    assert!(matches!(
        cache.consider(
            peer(1)?,
            reconnect,
            b"immutable body",
            Moment::from_nanos(100),
            Moment::from_nanos(200)
        )?,
        Decision::Rejected(EntryRejection::Deadline)
    ));
    assert!(matches!(
        cache.consider(
            peer(1)?,
            reconnect,
            b"changed body",
            Moment::from_nanos(2),
            Moment::from_nanos(200)
        )?,
        Decision::Rejected(EntryRejection::ChangedDuplicate)
    ));
    reconnect.source = CounterDeadline::new(50, 11)?;
    assert!(matches!(
        cache.consider(
            peer(1)?,
            reconnect,
            b"immutable body",
            Moment::from_nanos(2),
            Moment::from_nanos(200)
        )?,
        Decision::Rejected(EntryRejection::ChangedDuplicate)
    ));
    Ok(())
}
#[test]
fn pid_and_birth_key_are_exact_and_settled_outcome_cannot_change() -> Result<(), Box<dyn Error>> {
    let mut cache = cache()?;
    consider(&mut cache, 50)?;
    assert!(matches!(
        cache.consider(
            peer(2)?,
            header(50)?,
            b"immutable body",
            Moment::from_nanos(1),
            Moment::from_nanos(100)
        )?,
        Decision::New(_)
    ));
    let mut born_again = peer(1)?;
    born_again.birth += 1;
    assert!(matches!(
        cache.consider(
            born_again,
            header(50)?,
            b"immutable body",
            Moment::from_nanos(1),
            Moment::from_nanos(100)
        )?,
        Decision::New(_)
    ));
    cache.settle(
        peer(1)?,
        EntryRequestId::new(50)?,
        EntryOutcome::Rejected(EntryRejection::InvalidInput),
    )?;
    assert!(
        cache
            .settle(
                peer(1)?,
                EntryRequestId::new(50)?,
                EntryOutcome::Rejected(EntryRejection::Capacity)
            )
            .is_err()
    );
    Ok(())
}
#[test]
fn invalid_source_id_or_expired_admission_never_changes_cache() -> Result<(), Box<dyn Error>> {
    let mut cache = cache()?;
    let mut invalid = header(50)?;
    invalid.request = EntryRequestId::new(51)?;
    assert!(
        cache
            .consider(peer(1)?, invalid, b"body", Moment::from_nanos(1), Moment::from_nanos(100))
            .is_err()
    );
    assert!(
        cache
            .consider(
                peer(1)?,
                header(50)?,
                b"body",
                Moment::from_nanos(100),
                Moment::from_nanos(100)
            )
            .is_err()
    );
    assert!(cache.slots.iter().all(|slot| slot.record.is_none()));
    assert_eq!(cache.floor, 0);
    Ok(())
}
