// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project the retained helper's first operation through its existing process exit status.
use crate::ClaimError;

// Codes are interpreted only for the actual authenticated helper from the same signed
// installation. They diagnose an unresolved exit and never attest native settlement.
const PREFIX: u32 = 0xe0c1_0000;
const OPERATIONS: &[&str] = &[
    "unclassified",
    "profile_custody_changed_record",
    "profile_custody_record_missing",
    "profile_custody_cleanup",
    "profile_custody_complete_family",
    "profile_custody_contention",
    "profile_custody_writer",
    "profile_custody_path",
    "custody_retirement_control",
    "custody_complete_family",
    "custody_record_association",
    "custody_record_missing",
    "custody_original_control",
    "custody_original_cutoff",
    "custody_namespace_control",
    "close_handle",
    "claim_exclusion",
    "custody_parent_retire",
    "custody_command_session",
    "custody_command_replayed",
    "custody_command_stopping",
    "custody_command_phase",
    "custody_not_established",
    "custody_resource_repeated",
    "custody_registration_capacity",
    "custody_stop_resource",
    "custody_stop_cutoff",
    "profile_custody_owner",
    "profile_custody_stop_cutoff",
    "profile_custody_root_association",
    "profile_custody_changed_publication",
    "custody_read_correlation",
    "custody_write_correlation",
    "custody_write_shape",
    "custody_write_revoked",
    "custody_unexpected_observation",
    "custody_frame",
    "custody_initialize",
    "custody_source_namespace",
    "custody_parent_observation",
    "custody_self_membership",
    "custody_self_limits",
    "custody_not_independent",
    "custody_self_containment",
    "custody_claim_missing",
    "custody_cleanup_control",
    "custody_cleanup_missing",
    "custody_command",
    "custody_completion_owner",
    "custody_containment_owner",
    "custody_drain_bound",
    "custody_driver_original",
    "custody_driver_wake",
    "custody_establish_repeated",
    "custody_family_storage",
    "custody_finish_counter",
    "custody_finish_cutoff",
    "custody_helper_exit_foreign",
    "custody_helper_exit_job",
    "custody_helper_exit_membership",
    "custody_helper_exit_not_due",
    "custody_initialize_clock",
    "custody_lock_observation",
    "custody_namespace_cleanup",
    "custody_namespace_closed",
    "custody_operation",
    "custody_operation_exhausted",
    "custody_parent_containment",
    "custody_parent_identity",
    "custody_parent_output",
    "custody_parent_owner",
    "custody_pool_capacity",
    "custody_pool_closed",
    "custody_pool_slot",
    "custody_prepare_command",
    "custody_prepare_counter",
    "custody_prepare_cutoff",
    "custody_prepare_record",
    "custody_prepared_job",
    "custody_progress",
    "custody_receipt",
    "custody_receipt_owner",
    "custody_receive",
    "custody_registration_owner",
    "custody_registration_purpose",
    "custody_reply",
    "custody_reply_buffer",
    "custody_reply_capacity",
    "custody_reply_deadline",
    "custody_reply_owner",
    "custody_retirement",
    "custody_root_association",
    "custody_root_command",
    "custody_root_resource",
    "custody_session",
    "custody_slot_closed",
    "custody_slot_phase",
    "custody_slot_turn",
    "custody_source_access",
    "custody_source_access_closed",
    "custody_source_callback_pending",
    "custody_source_capacity",
    "custody_source_closed",
    "custody_source_failure",
    "custody_source_handles",
    "custody_source_key",
    "custody_source_reinitialized",
    "custody_stop_counter",
    "custody_suspended_root",
    "custody_transfer_index",
    "custody_transfer_missing",
    "custody_transfer_pending",
    "custody_transport_retirement",
    "custody_written_nonce",
    "custody_written_unsettled",
    "profile_custody_command_purpose",
    "profile_custody_counter",
    "profile_custody_cutoff",
    "profile_custody_directory",
    "profile_custody_exclusion",
    "profile_custody_family",
    "profile_custody_folder_identity",
    "profile_custody_prepare_command",
    "profile_custody_prepare_record",
    "profile_custody_prepared_job",
    "profile_custody_publication_mismatch",
    "profile_custody_publication_phase",
    "profile_custody_purpose",
    "profile_custody_repeated",
    "profile_custody_root_command",
    "profile_custody_stop_counter",
    "profile_custody_suspended_root",
    "profile_custody_written_unsettled",
];

const _: () = assert!(OPERATIONS.len() <= u16::MAX as usize);

/// Closed first-operation metadata from an actual helper's observed process exit.
/// This carries no original cause, process authority or cleanup/retirement proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendCustodyExitDiagnostic(u16);
impl BackendCustodyExitDiagnostic {
    /// Encode only the actual source owner's retained first cause without invoking it.
    /// An unmapped cause or an expired cleanup without a stored cause remains unclassified.
    pub(super) fn first(cause: Option<&ClaimError>) -> Self {
        let index = cause
            .and_then(|cause| {
                OPERATIONS.iter().position(|operation| *operation == cause.operation())
            })
            .unwrap_or(0);
        Self(index as u16)
    }
    /// Decode a diagnostic only after independently authenticating the actual same-build helper.
    /// Unknown native/application exit codes return `None` and remain failures.
    pub const fn from_exit_code(code: u32) -> Option<Self> {
        let index = (code & 0xffff) as u16;
        if code & 0xffff_0000 == PREFIX && (index as usize) < OPERATIONS.len() {
            Some(Self(index))
        } else {
            None
        }
    }
    /// Borrow a fixed source-defined operation label, with no paths, payloads or formatting.
    pub fn operation(self) -> &'static str {
        OPERATIONS.get(usize::from(self.0)).copied().unwrap_or(OPERATIONS[0])
    }
    /// Return the non-success status used by the authenticated unresolved-helper exit path.
    pub const fn exit_code(self) -> u32 {
        PREFIX | self.0 as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        fmt,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    #[test]
    fn every_closed_helper_operation_round_trips_without_accepting_unknown_native_status() {
        for (index, operation) in OPERATIONS.iter().enumerate() {
            let cause = ClaimError::invalid(operation);
            let value = BackendCustodyExitDiagnostic::first(Some(&cause));
            assert_eq!(usize::from(value.0), index);
            assert_eq!(
                BackendCustodyExitDiagnostic::from_exit_code(value.exit_code()),
                Some(value)
            );
            assert_eq!(value.operation(), *operation);
            assert_ne!(value.exit_code(), 0);
        }
        for code in [
            0,
            1,
            109,
            259,
            0xc000_0005,
            0x8007_006d,
            PREFIX - 1,
            PREFIX + OPERATIONS.len() as u32,
            u32::MAX,
        ] {
            assert_eq!(BackendCustodyExitDiagnostic::from_exit_code(code), None);
        }
    }
    struct Unknown {
        callbacks: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
    }
    impl fmt::Debug for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl fmt::Display for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl Error for Unknown {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            None
        }
    }
    impl Drop for Unknown {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn unclassified_operation_does_not_execute_or_release_original_cause() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let callbacks = Arc::new(AtomicUsize::new(0));
        let cause = ClaimError::new(
            "private/unmapped/path",
            Unknown { callbacks: callbacks.clone(), dropped: dropped.clone() },
        );
        let value = BackendCustodyExitDiagnostic::first(Some(&cause));
        assert_eq!(value.operation(), "unclassified");
        assert_eq!(value, BackendCustodyExitDiagnostic::first(None));
        assert!(!format!("{value:?}").contains("private"));
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
        drop(cause);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn secondary_native_close_failure_cannot_replace_original_operation_or_source()
    -> Result<(), Box<dyn Error>> {
        let first = ClaimError::new(
            "profile_custody_changed_record",
            std::io::Error::from_raw_os_error(13),
        );
        let source = first.source().map(std::ptr::from_ref);
        let later = ClaimError::new("close_handle", std::io::Error::from_raw_os_error(5));
        let joined = ClaimError::finish::<()>(Err(first.clone()), Err(later));
        let cause = joined.err().ok_or("Actual first failure lost")?;
        let value = BackendCustodyExitDiagnostic::first(Some(&cause));
        assert_eq!(value.operation(), "profile_custody_changed_record");
        assert!(cause.same_primary(&first));
        assert_eq!(cause.source().map(std::ptr::from_ref), source);
        assert_eq!(cause.cleanup_cause().map(ClaimError::operation), Some("close_handle"));
        assert_eq!(cause.win32_code(), Some(13));
        Ok(())
    }
}
