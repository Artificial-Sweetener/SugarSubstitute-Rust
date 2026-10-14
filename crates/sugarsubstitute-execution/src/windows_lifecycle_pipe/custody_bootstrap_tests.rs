// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject purpose, owner and original-counter substitutions before custody adoption.
use super::*;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Construct protocol facts only; this fixture supplies no native ownership.
fn data() -> TestResult<Data> {
    let key = BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([3; 32]),
            epoch: CoordinatorEpoch::new([4; 16])?,
        },
        BackendCustodySession::new(17)?,
    );
    let binding = PeerBinding::BackendCustody(key);
    Ok(Data {
        binding,
        pid: 101,
        birth: 303,
        session: 0,
        deadline: LifecyclePipeDeadline::new(9000, 1000)?,
        capability: [8; 32],
        name: name(binding),
    })
}

#[test]
fn custody_invocation_preserves_original_facts_and_rejects_other_purposes() -> TestResult {
    let original = data()?;
    let bytes = encode(&original)?;
    assert_eq!(&bytes[4..8], &[4, 0, 0, 4]);
    let decoded = decode(&bytes, original.deadline, Kind::BackendCustody)?;
    assert_eq!(decoded.binding, original.binding);
    assert_eq!(decoded.name, original.name);
    assert_eq!(decoded.pid, original.pid);
    assert_eq!(decoded.birth, original.birth);
    assert_eq!(decoded.deadline, original.deadline);
    for purpose in [
        Kind::Desktop,
        Kind::Diagnostic(DiagnosticRole::Storage),
        Kind::Diagnostic(DiagnosticRole::Symbolize),
        Kind::Diagnostic(DiagnosticRole::Reporter),
    ] {
        assert!(decode(&bytes, original.deadline, purpose).is_err());
    }
    assert!(decode_captured_base(&bytes, original.deadline, Kind::BackendCustody).is_err());
    Ok(())
}

#[test]
fn custody_bootstrap_rejects_foreign_owner_empty_secret_and_reserved_role() -> TestResult {
    let original = data()?;
    let valid = encode(&original)?;
    for offset in [8, 40, 56, 60, 64, 138, 144, 1023] {
        let mut altered = valid;
        altered[offset] ^= 1;
        assert!(decode(&altered, original.deadline, Kind::BackendCustody).is_err());
    }
    for absent in [64..72, 72..76, 80..88, 104..136] {
        let mut altered = valid;
        altered[absent].fill(0);
        assert!(decode(&altered, original.deadline, Kind::BackendCustody).is_err());
    }
    let mut no_secret = data()?;
    no_secret.capability.fill(0);
    assert!(encode(&no_secret).is_err());
    Ok(())
}

#[test]
fn custody_original_deadline_and_frequency_survive_without_renewal() -> TestResult {
    let original = data()?;
    let encoded = encode(&original)?;
    for changed in
        [LifecyclePipeDeadline::new(9001, 1000)?, LifecyclePipeDeadline::new(9000, 1001)?]
    {
        assert!(decode(&encoded, changed, Kind::BackendCustody).is_err());
    }
    assert_eq!(
        decode(&encoded, original.deadline, Kind::BackendCustody)?.deadline,
        original.deadline
    );
    Ok(())
}
