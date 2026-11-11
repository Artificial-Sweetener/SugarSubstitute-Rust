// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one lexical inherited Job and bounded delayed command input through admission.
use super::*;
/// Lexical dedicated child input; a numeric token grants no native/source authority.
pub struct InheritedRecoveryJobInput(usize);
impl InheritedRecoveryJobInput {
    /// Check native slot shape without adopting ownership or issuing native calls.
    /// # Errors
    /// Rejects zero and pseudo handles. The authenticated finite adapter adopts the real slot.
    pub fn from_token(value: usize) -> Result<Self, RegistryAdmissionError> {
        if value == 0 || value >= usize::MAX - 15 {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        Ok(Self(value))
    }
    /// Check-close this inherited input after genuine Root authentication, even if startup failed.
    /// Source provenance stays retained; the actual cleanup job checks its original Core.
    /// # Errors
    /// Returns the exact owned slot and actual first cutoff on finite cleanup admission failure.
    #[expect(
        clippy::result_large_err,
        reason = "Preserve exact original inherited input on rejection"
    )]
    pub fn submit_retirement(
        self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        origin: &AuthenticatedRecoveryOrigin,
    ) -> Result<RecoveryChildCreation, RecoveryChildCreationRejection> {
        let shared = Arc::new(Shared { state: Mutex::new(State::unstarted(Some(self.0))) });
        let mut owner = RecoveryChildCreation {
            shared,
            origin: origin.clone(),
            watch_completion: None,
            creation_completion: None,
            creation: None,
            cleanup: None,
            cleanup_completion: None,
            exit_verified: false,
            watch_admitted: false,
            cleanup_until: None,
            watch_id: (scope.id(), spec),
        };
        match owner.submit_unstarted_cleanup(execution, scope, spec) {
            Ok(()) => Ok(owner),
            Err(cause) => Err(RecoveryChildCreationRejection { cause, owner }),
        }
    }
}
/// Preserve every native input if the bounded command callback cannot be admitted.
pub struct RecoveryChildInputRejection {
    /// Original bounded-input rejection.
    pub cause: RegistryAdmissionError,
    /// Original lexical inherited Job, still responsible for actual close-only retirement.
    pub job: InheritedRecoveryJobInput,
    /// Original genuine capture/startup pins, unchanged and not consumed.
    pub captured: CapturedLifecycleStart,
}
impl std::fmt::Debug for RecoveryChildInputRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryChildInputRejection { original inputs retained }")
    }
}
impl std::fmt::Display for RecoveryChildInputRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery child input rejected")
    }
}
impl std::error::Error for RecoveryChildInputRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
/// Bounded once-only command construction, performed after the genuine section exists.
pub struct RecoveryChildInput {
    pub(super) shared: Arc<Shared>,
    pub(super) captured: CapturedLifecycleStart,
    pub(super) factory: Factory,
}
impl RecoveryChildInput {
    /// Retain the inherited slot and actual capture/startup pins until finite dispatch.
    /// The command factory runs once on the creation worker after section publication.
    /// # Errors
    /// Rejects callbacks larger than the fixed4KiB control partition before boxing.
    #[expect(
        clippy::result_large_err,
        reason = "Retain exact native inputs when the bounded callback rejects"
    )]
    pub fn new<F, E>(
        job: InheritedRecoveryJobInput,
        captured: CapturedLifecycleStart,
        factory: F,
    ) -> Result<Self, RecoveryChildInputRejection>
    where
        F: FnOnce(&RecoveryDesktopBootstrap, &JobContext) -> Result<WindowsLaunchInput, E>
            + Send
            + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        if size_of::<F>() > 4096 {
            return Err(RecoveryChildInputRejection {
                cause: RegistryAdmissionError::InvalidSpecification,
                job,
                captured,
            });
        }
        Ok(Self {
            shared: Arc::new(Shared { state: Mutex::new(State::unstarted(Some(job.0))) }),
            captured,
            factory: Box::new(move |section, context| {
                factory(section, context)
                    .map_err(|cause| RegistryError::new("recovery_child_command", cause))
            }),
        })
    }
    /// Return the unchanged four-MiB finite creation reservation, in bytes.
    pub const fn required_reservation_bytes(&self) -> u64 {
        CREATION_BYTES
    }
}
