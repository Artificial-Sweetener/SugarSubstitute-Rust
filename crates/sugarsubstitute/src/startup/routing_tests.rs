// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise retry identities against the genuine parent authorization owner.
use super::retry_identity;
use sugarsubstitute_application::supervision::{
    MainAuthorizationObservation, MainAuthorizationSession,
};
use sugarsubstitute_contracts::{
    execution::{Generation, Moment},
    lifecycle::{
        ClientBinding, ConnectionId, LeaseId, LeaseKey, RequestId, ResourceGeneration, ResourceId,
        ResourceKey,
    },
    startup::{LaunchId, MainIdentity},
};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn repeated_retry_advances_the_identity_accepted_after_actual_authorization_retirement() -> Result {
    let run = super::super::policy::tests::test_run()?;
    let binding = ClientBinding { run, connection: ConnectionId::new(1)? };
    let lease = LeaseKey {
        run,
        connection: binding.connection,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    };
    let mut identity =
        MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(7)? };
    let mut parent = MainAuthorizationSession::new(run);
    parent.begin(identity, lease, Moment::from_nanos(100))?;
    for request in 1..=3 {
        parent.retire(identity)?;
        parent.accept(
            binding,
            RequestId::new(request)?,
            MainAuthorizationObservation::MainRetired { identity },
            Moment::from_nanos(request),
        )?;
        parent.confirm_retired(identity)?;
        let next = retry_identity(identity)?;
        assert_eq!(next.run, run);
        assert!(next.launch.get() > identity.launch.get());
        assert!(next.navigation.get() > identity.navigation.get());
        parent.begin(next, lease, Moment::from_nanos(100))?;
        identity = next;
    }
    Ok(())
}

#[test]
fn retry_identity_exhaustion_never_wraps_either_generation() -> Result {
    let identity = MainIdentity {
        run: super::super::policy::tests::test_run()?,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    assert!(retry_identity(MainIdentity { launch: LaunchId::new(u64::MAX)?, ..identity }).is_err());
    assert!(
        retry_identity(MainIdentity { navigation: Generation::new(u64::MAX)?, ..identity })
            .is_err()
    );
    Ok(())
}

#[cfg(all(windows, feature = "native-composition"))]
#[test]
fn only_a_running_composition_hosted_sender_receives_native_canvas_operations() {
    use sugarsubstitute_desktop::MainHosting;
    assert!(super::grants_native_canvas(true, MainHosting::Composition));
    assert!(!super::grants_native_canvas(true, MainHosting::Windowed));
    assert!(!super::grants_native_canvas(false, MainHosting::Composition));
    assert!(!super::grants_native_canvas(false, MainHosting::Windowed));
}
