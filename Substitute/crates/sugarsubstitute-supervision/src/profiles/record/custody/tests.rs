// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject legacy/foreign witness authority while preserving exact160-byte profile identity.
use super::*;
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodySession,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};
#[test]
fn prospective_profile_v2_retains_same_store_run_and_rejects_legacy_held_authority()
-> Result<(), Box<dyn std::error::Error>> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let original = Record {
        account: super::super::account_identity(b"actual-format-test-sid"),
        folder: [4; 24],
        run,
        state: State::Held,
    };
    let value = CustodiedRecord::new(
        original,
        BackendCustodyKey::new(run.owner, BackendCustodySession::new(1)?),
        ProfileCustodyKey::new(run, 9)?,
    )?;
    assert_eq!(CustodiedRecord::decode(&value.encode())?, value);
    assert!(value.matches_account(b"actual-format-test-sid"));
    assert!(!value.matches_account(b"foreign-format-test-sid"));
    assert!(CustodiedRecord::decode(&original.encode()).is_err());
    let legacy_released = Record { state: State::Released, ..original };
    assert!(legacy_released.admits_successor(original));
    assert!(!original.admits_successor(original));
    assert_eq!(value.released().original.state, State::Released);
    for offset in [8, 9, 152, 159] {
        let mut bytes = value.encode();
        bytes[offset] = 255;
        assert!(CustodiedRecord::decode(&bytes).is_err());
    }
    for range in [136..144, 144..152] {
        let mut bytes = value.encode();
        bytes[range].fill(0);
        assert!(CustodiedRecord::decode(&bytes).is_err());
    }
    Ok(())
}
