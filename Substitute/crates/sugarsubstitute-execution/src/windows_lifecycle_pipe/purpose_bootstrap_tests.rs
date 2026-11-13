// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject purpose, role, binding, secret, endpoint and budget substitutions before client effects.
use super::*;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn data(role: DiagnosticRole) -> TestResult<Data> {
    let key = DiagnosticKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        role,
        DiagnosticSessionId::new(9)?,
    );
    Ok(Data {
        binding: PeerBinding::Purpose(PurposeKey::Diagnostic(key)),
        pid: 71,
        birth: 99,
        session: 0,
        deadline: LifecyclePipeDeadline::new(1000, 100)?,
        capability: [7; 32],
        name: name(PeerBinding::Purpose(PurposeKey::Diagnostic(key))),
    })
}
#[test]
fn diagnostic_bootstrap_exact_layout_roundtrip_uses_fixed_invocation_role() -> TestResult {
    for role in [DiagnosticRole::Storage, DiagnosticRole::Symbolize, DiagnosticRole::Reporter] {
        let source = data(role)?;
        let bytes = encode(&source)?;
        assert_eq!(&bytes[4..8], &[2, 0, 0, 4]);
        assert_eq!(&bytes[56..60], &2u32.to_le_bytes());
        assert_eq!(&bytes[60..64], &role_code(role).to_le_bytes());
        assert!(bytes[656..].iter().all(|b| *b == 0));
        let decoded = decode(&bytes, source.deadline, Kind::Purpose(Purpose::Diagnostic(role)))?;
        assert_eq!(decoded.binding, source.binding);
        assert_eq!(decoded.session, 0);
        assert_eq!(decoded.name, source.name);
        assert_eq!(decoded.capability, source.capability);
    }
    Ok(())
}
fn attempt_data(attempt: u64) -> TestResult<Data> {
    let channel = UpdateAttemptChannel::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        UpdateAttemptKey::new(std::num::NonZeroU64::new(attempt).ok_or("Zero attempt")?),
    );
    let binding = PeerBinding::Purpose(PurposeKey::UpdateAttempt(channel));
    Ok(Data {
        binding,
        pid: 71,
        birth: 99,
        session: 0,
        deadline: LifecyclePipeDeadline::new(1000, 100)?,
        capability: [7; 32],
        name: name(binding),
    })
}
#[test]
fn update_attempt_bootstrap_roundtrips_on_its_own_endpoint() -> TestResult {
    let source = attempt_data(42)?;
    let bytes = encode(&source)?;
    assert_eq!(&bytes[4..8], &[2, 0, 0, 4]);
    assert_eq!(&bytes[56..60], &UPDATE_ATTEMPT_PURPOSE.to_le_bytes());
    assert_eq!(&bytes[60..64], &[0; 4]);
    assert_eq!(&bytes[64..72], &42u64.to_le_bytes());
    let endpoint = String::from_utf16(&source.name[..source.name.len() - 1])?;
    assert!(endpoint.starts_with("\\\\.\\pipe\\SugarSubstitute.update-attempt.v1."));
    assert_ne!(source.name, data(DiagnosticRole::Storage)?.name);
    let decoded = decode(&bytes, source.deadline, Kind::Purpose(Purpose::UpdateAttempt))?;
    assert_eq!(decoded.binding, source.binding);
    assert_eq!(decoded.capability, source.capability);
    Ok(())
}
#[test]
fn update_attempt_and_diagnostic_bootstraps_never_decode_as_each_other() -> TestResult {
    let attempt = attempt_data(42)?;
    let attempt_bytes = encode(&attempt)?;
    for role in [DiagnosticRole::Storage, DiagnosticRole::Symbolize, DiagnosticRole::Reporter] {
        let kind = Kind::Purpose(Purpose::Diagnostic(role));
        assert!(decode(&attempt_bytes, attempt.deadline, kind).is_err());
        let diagnostic = data(role)?;
        let bytes = encode(&diagnostic)?;
        let kind = Kind::Purpose(Purpose::UpdateAttempt);
        assert!(decode(&bytes, diagnostic.deadline, kind).is_err());
    }
    for offset in [56, 60, 64, 144] {
        let mut bytes = attempt_bytes;
        bytes[offset] ^= 1;
        let kind = Kind::Purpose(Purpose::UpdateAttempt);
        assert!(decode(&bytes, attempt.deadline, kind).is_err(), "accepted {offset}");
    }
    let mut zero = attempt_bytes;
    zero[64..72].fill(0);
    assert!(decode(&zero, attempt.deadline, Kind::Purpose(Purpose::UpdateAttempt)).is_err());
    Ok(())
}
#[test]
fn diagnostic_wrong_invocation_role_and_desktop_bootstrap_never_select_authority() -> TestResult {
    let source = data(DiagnosticRole::Storage)?;
    let bytes = encode(&source)?;
    for kind in [
        Kind::Desktop,
        Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Symbolize)),
        Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Reporter)),
    ] {
        assert!(decode(&bytes, source.deadline, kind).is_err())
    }
    Ok(())
}
#[test]
fn diagnostic_reserved_bytes_scope_session_endpoint_and_capability_fail_closed() -> TestResult {
    let source = data(DiagnosticRole::Storage)?;
    let valid = encode(&source)?;
    for offset in [0, 4, 6, 8, 40, 56, 60, 64, 88, 96, 136, 138, 144, 655, 656, 1023] {
        let mut bytes = valid;
        bytes[offset] ^= 1;
        assert!(
            decode(
                &bytes,
                source.deadline,
                Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Storage))
            )
            .is_err(),
            "accepted {offset}"
        );
    }
    for range in [64..72, 72..76, 80..88, 104..136] {
        let mut bytes = valid;
        bytes[range].fill(0);
        assert!(
            decode(
                &bytes,
                source.deadline,
                Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Storage))
            )
            .is_err()
        );
    }
    Ok(())
}
#[test]
fn diagnostic_original_budget_and_nonzero_nonce_cannot_be_renewed() -> TestResult {
    let mut source = data(DiagnosticRole::Storage)?;
    let valid = encode(&source)?;
    assert!(
        decode(
            &valid,
            LifecyclePipeDeadline::new(1001, 100)?,
            Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Storage))
        )
        .is_err()
    );
    assert!(
        decode(
            &valid,
            LifecyclePipeDeadline::new(1000, 101)?,
            Kind::Purpose(Purpose::Diagnostic(DiagnosticRole::Storage))
        )
        .is_err()
    );
    source.capability.fill(0);
    assert!(encode(&source).is_err());
    Ok(())
}
