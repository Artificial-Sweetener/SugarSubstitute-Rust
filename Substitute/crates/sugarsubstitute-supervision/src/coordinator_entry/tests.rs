// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve schema-one election policy without native effects.
use super::{
    election::FreshRecordOrigin,
    native::{self, Elected},
};
use crate::{
    ClaimError,
    durable_storage::protected::record_file::RecordRead,
    native_claims::{Record, RecordState},
};
use sugarsubstitute_contracts::{
    coordinator_entry::EntryOwnerDegradation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn missing_and_empty_require_the_actual_retained_exclusion_inode() -> TestResult {
    let ns = NamespaceId::new([1; 32]);
    assert!(matches!(native::classify(RecordRead::Missing, ns, None)?, Elected::Contended(None)));
    assert!(matches!(native::classify(RecordRead::Empty, ns, None)?, Elected::Contended(None)));
    assert!(
        matches!(native::classify(RecordRead::Missing,ns,Some([9;24]))?,Elected::Fresh {origin:FreshRecordOrigin::Missing,inode} if inode==[9;24])
    );
    assert!(
        matches!(native::classify(RecordRead::Empty,ns,Some([9;24]))?,Elected::Fresh {origin:FreshRecordOrigin::Empty,inode} if inode==[9;24])
    );
    Ok(())
}
#[test]
fn released_record_is_exact_ordinary_provenance_and_never_fresh() -> TestResult {
    let ns = NamespaceId::new([1; 32]);
    let mut record = Record::coordinator(CoordinatorKey {
        namespace: ns,
        epoch: CoordinatorEpoch::new([2; 16])?,
    });
    record.state = RecordState::Released;
    let bytes = record.encode();
    assert!(
        matches!(native::classify(RecordRead::Present(bytes),ns,Some([9;24]))?,Elected::OrdinaryReleased {record,inode} if record==bytes && inode==[9;24])
    );
    assert!(matches!(
        native::classify(RecordRead::Present(bytes), ns, None)?,
        Elected::Contended(None)
    ));
    for offset in [8, 60, 68, 76, 84, 116] {
        let mut changed = bytes;
        changed[offset] ^= 1;
        assert!(matches!(
            native::classify(RecordRead::Present(changed), ns, Some([9; 24]))?,
            Elected::Degraded(EntryOwnerDegradation::RecordUnknown)
        ));
    }
    assert!(matches!(
        native::classify(RecordRead::Present(bytes), NamespaceId::new([3; 32]), Some([9; 24]))?,
        Elected::Degraded(EntryOwnerDegradation::RecordUnknown)
    ));
    Ok(())
}
#[test]
fn held_record_supplies_only_a_candidate_epoch_without_granting_election() -> TestResult {
    let ns = NamespaceId::new([1; 32]);
    let epoch = CoordinatorEpoch::new([2; 16])?;
    let record = Record::coordinator(CoordinatorKey { namespace: ns, epoch });
    for inode in [None, Some([9; 24])] {
        assert!(
            matches!(native::classify(RecordRead::Present(record.encode()),ns,inode)?,Elected::Candidate{epoch:actual,holder:None} if actual==epoch)
        );
    }
    Ok(())
}
/// A record names the coordinator process that holds it, held or released, so a launch that
/// cannot reach that coordinator still knows which process to ask whether it responds.
#[test]
fn a_record_names_its_holding_coordinator_while_held_and_after_release() -> TestResult {
    let ns = NamespaceId::new([1; 32]);
    let epoch = CoordinatorEpoch::new([2; 16])?;
    let holder = super::EntryHolder { process_id: 4242, creation_time_100ns: 1_330_000 };
    let mut record = Record::coordinator(CoordinatorKey { namespace: ns, epoch }).held_by(holder);
    assert!(matches!(
        native::classify(RecordRead::Present(record.encode()), ns, None)?,
        Elected::Candidate { epoch: actual, holder: Some(named) } if actual == epoch && named == holder
    ));
    record.state = RecordState::Released;
    assert!(matches!(
        native::classify(RecordRead::Present(record.encode()), ns, None)?,
        Elected::Contended(Some(named)) if named == holder
    ));
    Ok(())
}
#[test]
fn only_actual_would_block_is_recoverable_exclusion_contention() {
    assert!(native::is_contention(&ClaimError::new(
        "claim_exclusion",
        std::fs::TryLockError::WouldBlock
    )));
    assert!(!native::is_contention(&ClaimError::new(
        "claim_exclusion",
        std::fs::TryLockError::Error(std::io::Error::other("original native failure"))
    )));
    assert!(!native::is_contention(&ClaimError::new(
        "claim_exclusion",
        std::io::Error::from(std::io::ErrorKind::WouldBlock)
    )));
    assert!(!native::is_contention(&ClaimError::new(
        "open_claim",
        std::fs::TryLockError::WouldBlock
    )));
}
