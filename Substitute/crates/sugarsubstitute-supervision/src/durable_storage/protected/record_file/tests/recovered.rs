// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move actual retained fixture custody through the real stale initializer and exact CAS.
use super::*;
use crate::{
    coordinator_entry::fixture::{initialization_guard_held, install_recovery},
    native_claims::{CoordinatorPredecessor, Record, RecordState, prepare_coordinator_release},
};
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey};
fn generation(epoch: u8) -> Result<Record, ClaimError> {
    Ok(Record::coordinator(CoordinatorKey {
        namespace: resolved().namespace,
        epoch: CoordinatorEpoch::new([epoch; 16])
            .map_err(|cause| ClaimError::new("test_epoch", cause))?,
    }))
}
#[test]
fn recovered_initializer_moves_same_charged_exclusion_and_keeps_entry_guard_through_cas()
-> TestResult {
    let previous = generation(2)?.encode();
    let desired = generation(3)?.encode();
    let mut fixture = PublicationFixture::new(ElectionRecordOrigin::RecoveredHeld {
        record: previous,
        coordinator_inode: [11; 24],
    })?;
    let proof = fixture.retained_proof()?;
    let fake = Fake::new(RecordRead::Present(previous));
    let external = fake.clone();
    let coordinator = fixture.entry.fake.clone();
    let spec = fixture.entry.spec(Moment::from_nanos(15_000_000_000))?;
    let mut job=fixture.entry.client.submit_blocking(&fixture.entry.scope,spec,move|context|->Result<(),ClaimError>{
        let mut fake=fake;let source=Arc::new(Mutex::new(None));let inode=adopt(&source,&mut fake,&context)?;
        coordinator.observe_coordinator(inode);
        install_recovery(&proof,&mut lock(&source))?;
        assert!(lock(&source).is_none());
        let mut guard=proof.lock_for_initialization(&context,&resolved())?;
        let mut root=None;guard.adopt_recovered_coordinator(&mut root)?;
        assert!(guard.adopt_recovered_coordinator(&mut root).is_err());
        let file=root.as_mut().ok_or_else(||ClaimError::invalid("handoff_missing"))?;
        assert!(file.is_locked());
        assert!(context.memory().claim(crate::coordinator_entry::ENTRY_PREPARATION_BYTES).is_err());
                assert!(initialization_guard_held(&proof));
        coordinator.observe_coordinator([99;24]);
        assert!(guard.publication(file,inode).is_err());
        coordinator.observe_coordinator(inode);
        let permission=guard.publication(file,inode)?;
        publication_core::publish_with_boundary(file,&desired,PublicationAuthority::ElectedCoordinator(&permission),&context,|old|permission.validate(file,old),&mut fake)?;
        assert!(matches!(fake.read_shape(file,None,&mut||Ok(()), &crate::durable_storage::protected::exclusion::operation::Operation::begin(file)?)?,RecordRead::Present(bytes) if bytes==desired));
                assert!(initialization_guard_held(&proof));
        assert!(lock(&file.stage).is_none());
        drop(permission);
        file.retire_fixture(inode,&mut fake,&context)?;coordinator.close_coordinator();
        drop(guard);drop(root);
        assert_eq!(context.memory().claim(crate::coordinator_entry::ENTRY_PREPARATION_BYTES).map_err(|cause|ClaimError::new("claim_not_returned",cause))?.bytes(),crate::coordinator_entry::ENTRY_PREPARATION_BYTES);
        assert_eq!(fake.live(),0);
        Ok(())
    })?;
    let completion = fixture.entry.consume_job(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
    drop((completion, job));
    assert_eq!(external.live(), 0);
    finish(&mut fixture)?;
    Ok(())
}
#[test]
fn recovered_original_held_is_preserved_only_before_actual_activation() -> TestResult {
    let old = generation(2)?;
    let bytes = old.encode();
    let mut own = Some(generation(3)?);
    assert!(!prepare_coordinator_release(
        PublicationRetirement::Preserved(RecordRead::Present(bytes)),
        &mut own,
        Some(CoordinatorPredecessor::RecoveredHeld(&bytes))
    )?);
    assert_eq!(own.as_ref().map(Record::encode), Some(generation(3)?.encode()));
    assert!(
        prepare_coordinator_release(
            PublicationRetirement::Activated(bytes),
            &mut own,
            Some(CoordinatorPredecessor::RecoveredHeld(&bytes))
        )
        .is_err()
    );
    let activated = own.as_ref().ok_or_else(|| ClaimError::invalid("own_missing"))?.encode();
    assert!(prepare_coordinator_release(
        PublicationRetirement::Activated(activated),
        &mut own,
        Some(CoordinatorPredecessor::RecoveredHeld(&bytes))
    )?);
    assert_eq!(own.as_ref().map(|value| value.state), Some(RecordState::Released));
    let released = own.as_ref().ok_or_else(|| ClaimError::invalid("own_missing"))?.encode();
    assert!(!prepare_coordinator_release(
        PublicationRetirement::Activated(released),
        &mut own,
        Some(CoordinatorPredecessor::RecoveredHeld(&bytes))
    )?);
    Ok(())
}
