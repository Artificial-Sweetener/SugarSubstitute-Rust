// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply exact kernel credential comparisons separately from their finite query effects.
use super::*;
pub(crate) fn capability(
    candidate: &[u8; 32],
    expected: &[u8; 32],
) -> Result<(), LifecyclePipeFailure> {
    let difference =
        candidate.iter().zip(expected).fold(0u8, |difference, (a, b)| difference | (a ^ b));
    if difference != 0 {
        return Err(failure("pipe_capability_mismatch"));
    }
    Ok(())
}
pub(crate) fn address(
    pid: u32,
    session: u32,
    expected: (u32, u64),
    expected_session: u32,
) -> Result<(), LifecyclePipeFailure> {
    if pid == 0 || expected.1 == 0 || pid != expected.0 || session != expected_session {
        return Err(failure("pipe_peer_pid_session_mismatch"));
    }
    Ok(())
}
pub(crate) fn birth(actual: (u32, u64), expected: (u32, u64)) -> Result<(), LifecyclePipeFailure> {
    if actual != expected {
        return Err(failure("pipe_peer_birth_mismatch"));
    }
    Ok(())
}
pub(crate) fn identity(
    actual: security::Identity,
    own: security::Identity,
    stage: &'static str,
) -> Result<(), LifecyclePipeFailure> {
    if actual != own {
        return Err(failure(stage));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn peer_address_and_birth_reject_pid_reuse_foreign_session_and_zero_facts() {
        assert!(address(71, 1, (71, 99), 1).is_ok());
        for (pid, session, expected) in
            [(72, 1, (71, 99)), (71, 2, (71, 99)), (0, 1, (0, 99)), (71, 1, (71, 0))]
        {
            assert!(address(pid, session, expected, 1).is_err());
        }
        assert!(birth((71, 99), (71, 99)).is_ok());
        assert!(birth((71, 100), (71, 99)).is_err());
        assert!(birth((72, 99), (71, 99)).is_err());
    }
    #[test]
    fn every_capability_byte_is_required_without_secret_diagnostics() {
        let expected = [7; 32];
        assert!(capability(&expected, &expected).is_ok());
        for index in 0..32 {
            let mut candidate = expected;
            candidate[index] ^= 1;
            let error = capability(&candidate, &expected).err();
            assert!(error.is_some());
            assert!(!format!("{error:?}").contains("[7"));
        }
    }
    #[test]
    fn primary_and_message_identity_require_account_logon_and_session() {
        let own = security::Identity::fixture(1, 2, 3);
        assert!(identity(own, own, "credential_test").is_ok());
        for actual in [
            security::Identity::fixture(4, 2, 3),
            security::Identity::fixture(1, 4, 3),
            security::Identity::fixture(1, 2, 4),
        ] {
            assert!(identity(actual, own, "credential_test").is_err());
        }
    }
}
