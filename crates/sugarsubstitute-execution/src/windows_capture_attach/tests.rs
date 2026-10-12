// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual kernel-fact policy with isolated finite boundary observations.
use super::{native::*, *};
use crate::{
    capture_attach::test_support, native_pipe::security::Identity, runtime_test_support::TestResult,
};
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
