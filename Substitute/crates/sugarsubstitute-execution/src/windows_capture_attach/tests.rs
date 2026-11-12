// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual kernel-fact policy with isolated finite boundary observations.
use super::{native::*, *};
use crate::{
    capture_attach::test_support, native_pipe::security::Identity, runtime_test_support::TestResult,
};
/// A valid ping may outlive its connection, but never its authenticated handle/process facts.
#[test]
fn completed_ping_retains_connected_peer_and_rejects_changed_or_dead_custody() -> TestResult {
    use std::{cell::Cell, error::Error};
    let expected = test_support::expected()?;
    let own = ProcessProof {
        facts: expected.creator(),
        identity: Identity::fixture(1, 2, 3),
        live: true,
    };
    let proofs = [own, own, ProcessProof { facts: expected.handler(), ..own }];
    let queries = Cell::new(0);
    let connected = Cell::new(true);
    let peer = ConnectedPeer::authenticate(expected, proofs, 71, || {
        assert!(connected.get());
        queries.set(queries.get() + 1);
        Ok(expected.creator().process_id())
    })?;
    // The actual Crashpad service writes the zero ping reply, then disconnects.
    connected.set(false);
    peer.revalidate(expected, proofs, 71)?;
    assert_eq!(queries.get(), 1);
    assert!(peer.revalidate(expected, proofs, 72).is_err());
    for index in 0..3 {
        let mut changed = proofs;
        changed[index].live = false;
        assert!(peer.revalidate(expected, changed, 71).is_err());
        changed = proofs;
        changed[index].facts = CaptureProcessFacts::new(91, 92)?;
        assert!(peer.revalidate(expected, changed, 71).is_err());
        changed = proofs;
        changed[index].identity = Identity::fixture(9, 2, 3);
        assert!(peer.revalidate(expected, changed, 71).is_err());
    }
    assert!(ConnectedPeer::authenticate(expected, proofs, 71, || Ok(91)).is_err());
    let original = crate::native::NativeObservationError::new(
        "capture_pipe_server",
        std::io::Error::from_raw_os_error(233),
    );
    let failure = match ConnectedPeer::authenticate(expected, proofs, 71, || Err(original.clone()))
    {
        Err(failure) => failure,
        Ok(_) => return Err(std::io::Error::other("Failed peer query authenticated").into()),
    };
    assert_eq!(failure.stage(), original.stage());
    let actual_cause = failure.source().ok_or("Original query cause missing")?;
    let original_cause = original.source().ok_or("Original query cause missing")?;
    assert!(std::ptr::eq(actual_cause, original_cause));
    Ok(())
}
#[test]
fn actual_creator_inherited_endpoint_policy_rejects_every_foreign_or_dead_pair() -> TestResult {
    let expected = test_support::expected()?;
    let own = ProcessProof {
        facts: expected.creator(),
        identity: Identity::fixture(1, 2, 3),
        live: true,
    };
    let creator = own;
    let handler = ProcessProof { facts: expected.handler(), ..own };
    assert!(verify_proof(expected, own, creator, handler, expected.creator().process_id()).is_ok());
    assert!(verify_proof(expected, own, creator, handler, expected.handler().process_id()).is_ok());
    assert!(verify_proof(expected, own, creator, handler, 91).is_err());
    for identity in
        [Identity::fixture(9, 2, 3), Identity::fixture(1, 9, 3), Identity::fixture(1, 2, 9)]
    {
        assert!(
            verify_proof(expected, own, ProcessProof { identity, ..creator }, handler, 3).is_err()
        );
        assert!(
            verify_proof(expected, own, creator, ProcessProof { identity, ..handler }, 3).is_err()
        );
    }
    for facts in [CaptureProcessFacts::new(3, 91)?, CaptureProcessFacts::new(92, 4)?] {
        assert!(
            verify_proof(expected, own, ProcessProof { facts, ..creator }, handler, 3).is_err()
        );
    }
    for facts in [CaptureProcessFacts::new(5, 91)?, CaptureProcessFacts::new(92, 6)?] {
        assert!(
            verify_proof(expected, own, creator, ProcessProof { facts, ..handler }, 3).is_err()
        );
    }
    assert!(
        verify_proof(expected, ProcessProof { live: false, ..own }, creator, handler, 3).is_err()
    );
    assert!(
        verify_proof(expected, own, ProcessProof { live: false, ..creator }, handler, 3).is_err()
    );
    assert!(
        verify_proof(expected, own, creator, ProcessProof { live: false, ..handler }, 3).is_err()
    );
    Ok(())
}
