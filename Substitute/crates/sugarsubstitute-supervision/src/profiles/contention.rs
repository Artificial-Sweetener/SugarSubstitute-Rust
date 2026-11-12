// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the profile-specific meaning of native exclusion contention.
use crate::ClaimError;
use std::{error::Error, fmt};

/// Another owner currently holds the requested profile's native exclusion.
///
/// This observation grants no authority to close that owner, remove its files or
/// admit a browser. The original native cause remains available for diagnostics.
#[derive(Debug)]
pub struct ProfileInUse {
    cause: ClaimError,
    request: Option<crate::profile_conflict::ConflictRequest>,
}

impl ProfileInUse {
    /// Copy inert protected-profile facts for a separately admitted conflict resolution session.
    pub fn request(
        mut cause: &(dyn Error + 'static),
    ) -> Option<crate::profile_conflict::ConflictRequest> {
        for _ in 0..32 {
            if cause
                .downcast_ref::<ClaimError>()
                .is_some_and(|claim| claim.cleanup_cause().is_some())
            {
                return None;
            }
            if let Some(busy) = cause.downcast_ref::<Self>() {
                return busy.request.clone();
            }
            cause = cause.source()?;
        }
        None
    }
    /// Recognize typed contention through at most 32 original error sources.
    /// Cleanup errors keep their full diagnostic presentation instead of a busy notice.
    pub fn in_error_chain(mut cause: &(dyn Error + 'static)) -> bool {
        for _ in 0..32 {
            if cause
                .downcast_ref::<ClaimError>()
                .is_some_and(|claim| claim.cleanup_cause().is_some())
            {
                return false;
            }
            if cause.is::<Self>() {
                return true;
            }
            let Some(source) = cause.source() else {
                return false;
            };
            cause = source;
        }
        false
    }
}

impl fmt::Display for ProfileInUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The requested browser profile is already in use")
    }
}

impl Error for ProfileInUse {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Classify only a proven busy profile exclusion, preserving all other failures unchanged.
pub(super) fn classify(cause: ClaimError) -> ClaimError {
    if cause.operation() == "claim_exclusion"
        && cause.cleanup_cause().is_none()
        && matches!(
            cause.source().and_then(|source| source.downcast_ref::<std::fs::TryLockError>()),
            Some(std::fs::TryLockError::WouldBlock)
        )
    {
        ClaimError::new("profile_in_use", ProfileInUse { cause, request: None })
    } else {
        cause
    }
}

/// Attach only canonical identity already established by the protected profile owner.
pub(super) fn at_profile(
    cause: ClaimError,
    path: &std::path::Path,
    folder: [u8; 24],
    account: [u8; 32],
) -> ClaimError {
    let classified = classify(cause);
    if let Some(busy) = classified.source().and_then(|source| source.downcast_ref::<ProfileInUse>())
    {
        ClaimError::new(
            "profile_in_use",
            ProfileInUse {
                cause: busy.cause.clone(),
                request: Some(crate::profile_conflict::ConflictRequest {
                    path: path.to_path_buf(),
                    folder,
                    account,
                }),
            },
        )
    } else {
        classified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_profile_keeps_the_exact_native_cause_and_changes_no_other_failure() {
        let original = ClaimError::new("claim_exclusion", std::fs::TryLockError::WouldBlock);
        let classified = classify(original.clone());
        assert!(ProfileInUse::in_error_chain(&classified));
        let busy = classified.source().and_then(|cause| cause.downcast_ref::<ProfileInUse>());
        assert!(busy.is_some_and(|busy| busy.cause.same_primary(&original)));
        for original in [
            ClaimError::new("other_lock", std::fs::TryLockError::WouldBlock),
            ClaimError::new(
                "claim_exclusion",
                std::io::Error::from(std::io::ErrorKind::WouldBlock),
            ),
            ClaimError::new(
                "claim_exclusion",
                std::fs::TryLockError::Error(std::io::Error::from_raw_os_error(5)),
            ),
        ] {
            assert!(!ProfileInUse::in_error_chain(&original));
            assert!(classify(original.clone()).same_primary(&original));
        }
        let original =
            ClaimError::finish::<()>(Err(original), Err(ClaimError::invalid("close_failed"))).err();
        assert!(original.is_some_and(|original| {
            assert!(!ProfileInUse::in_error_chain(&original));
            classify(original.clone()).same_observation(&original)
        }));
    }

    #[test]
    fn typed_contention_survives_wrapping_but_never_hides_later_cleanup_failure() {
        let busy = classify(ClaimError::new("claim_exclusion", std::fs::TryLockError::WouldBlock));
        let wrapped = ClaimError::new("entry", busy.clone());
        assert!(ProfileInUse::in_error_chain(&wrapped));
        let failed =
            ClaimError::finish::<()>(Err(busy), Err(ClaimError::invalid("close_failed"))).err();
        assert!(failed.is_some_and(|cause| !ProfileInUse::in_error_chain(&cause)));
        assert!(!ProfileInUse::in_error_chain(&std::io::Error::other("profile_in_use")));
    }
}
