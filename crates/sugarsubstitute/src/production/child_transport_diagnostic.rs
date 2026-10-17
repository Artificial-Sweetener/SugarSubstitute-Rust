// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only closed pre-UI lifecycle operation metadata through existing owned stderr.
use sugarsubstitute_execution::windows::NativeObservationError;

const STAGES: &[&str] = &[
    "unclassified",
    "capture_close_event",
    "capture_event_accounting",
    "capture_event_cleanup_proof",
    "captured_bootstrap_base",
    "captured_bootstrap_build",
    "captured_bootstrap_capability",
    "captured_bootstrap_endpoint",
    "captured_bootstrap_field",
    "captured_bootstrap_kind",
    "captured_bootstrap_matching_facts",
    "captured_bootstrap_parent_binding",
    "captured_bootstrap_schema",
    "captured_child_base",
    "captured_child_facts",
    "captured_child_identity",
    "captured_child_original_until",
    "captured_child_replay",
    "captured_child_self_alias",
    "captured_child_target",
    "captured_clear_process_inheritance",
    "captured_clear_section_inherit",
    "captured_client_deadline",
    "captured_input_original_cleanup",
    "captured_input_returned",
    "captured_live_map_loan",
    "captured_mapping_adopt",
    "captured_mapping_claim_missing",
    "captured_mapping_duplicate",
    "captured_mapping_extent",
    "captured_mapping_layout",
    "captured_mapping_loan",
    "captured_mapping_owner",
    "captured_mapping_purpose",
    "captured_native_layout",
    "captured_native_scope",
    "captured_original_counter",
    "captured_parent_current",
    "captured_parent_owner",
    "captured_parent_peer_mismatch",
    "captured_section_extent",
    "captured_startup_body",
    "captured_startup_bound",
    "captured_startup_extent",
    "captured_startup_prefix",
    "captured_startup_purpose_payload",
    "pipe_authenticate_before_read",
    "pipe_authenticated_binding",
    "pipe_bootstrap_bounds",
    "pipe_bootstrap_capability",
    "pipe_bootstrap_custody_purpose",
    "pipe_bootstrap_custody_session",
    "pipe_bootstrap_deadline",
    "pipe_bootstrap_diagnostic_role",
    "pipe_bootstrap_diagnostic_session",
    "pipe_bootstrap_endpoint",
    "pipe_bootstrap_epoch",
    "pipe_bootstrap_field",
    "pipe_bootstrap_generation",
    "pipe_bootstrap_instance",
    "pipe_bootstrap_name_bound",
    "pipe_bootstrap_owner",
    "pipe_bootstrap_process",
    "pipe_bootstrap_session",
    "pipe_bootstrap_version",
    "pipe_buffer_claim",
    "pipe_cancel_io",
    "pipe_capability_mismatch",
    "pipe_capability_random",
    "pipe_clear_bootstrap_inherit",
    "pipe_client_deadline",
    "pipe_client_message_mode",
    "pipe_client_pid",
    "pipe_client_session",
    "pipe_completion",
    "pipe_connect_phase",
    "pipe_create_bootstrap",
    "pipe_create_event",
    "pipe_current_admission",
    "pipe_expired_bootstrap",
    "pipe_handshake_deadline",
    "pipe_identity_owner",
    "pipe_impersonate",
    "pipe_live_event_reuse",
    "pipe_live_map_reuse",
    "pipe_mapping_owner",
    "pipe_message_bound",
    "pipe_metadata_already_installed",
    "pipe_metadata_claim",
    "pipe_metadata_owner",
    "pipe_missing_lease",
    "pipe_open_actual_peer",
    "pipe_open_local_client",
    "pipe_open_message_token",
    "pipe_operation_owner",
    "pipe_partial_write",
    "pipe_passive_identity",
    "pipe_peer_birth_mismatch",
    "pipe_peer_exited",
    "pipe_peer_liveness",
    "pipe_peer_pid_session_mismatch",
    "pipe_prepared_binding",
    "pipe_process_birth",
    "pipe_process_id",
    "pipe_read_owner",
    "pipe_read_phase",
    "pipe_reset_event",
    "pipe_revert_identity",
    "pipe_security_descriptor",
    "pipe_server_binding",
    "pipe_server_pid",
    "pipe_server_session",
    "pipe_token_information",
    "pipe_unmap_bootstrap",
    "pipe_verify_before_read",
    "pipe_wait_owner",
    "pipe_write_admission",
    "pipe_write_issue_phase",
    "pipe_write_owner",
    "recovery_bootstrap_binding",
    "recovery_bootstrap_key",
    "recovery_bootstrap_purpose",
    "recovery_pair_input_slots",
    "recovery_pair_wrong_purpose",
    "recovery_root_section_authenticated_binding",
    "recovery_root_section_binding",
    "recovery_root_section_counter",
    "recovery_root_section_frequency",
    "recovery_root_section_missing",
    "recovery_root_section_purpose",
    "recovery_root_section_replay",
    "recovery_section_actual_creator",
    "recovery_section_core",
    "recovery_section_endpoint",
    "recovery_section_frequency",
    "recovery_section_identity",
    "recovery_section_input",
    "recovery_section_layout",
    "recovery_section_not_ready",
    "recovery_section_owner",
    "recovery_section_processes",
    "recovery_section_replay",
    "recovery_section_retirement",
    "recovery_section_session",
    "recovery_section_source",
    "recovery_section_startup",
    "recovery_ui_key",
    "recovery_ui_original_counter",
    "recovery_ui_purpose",
    "recovery_ui_root_binding",
];

/// Inert source-defined operation/code data; never native or peer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Record {
    pub(super) stage: &'static str,
    pub(super) win32: Option<u32>,
}
impl Record {
    /// Inspect only the native owner's own stage and concrete numeric leaf, without callbacks.
    fn actual(cause: &NativeObservationError) -> Self {
        Self { stage: stage(cause.stage()).unwrap_or("unclassified"), win32: cause.win32_code() }
    }
}
fn stage(value: &str) -> Option<&'static str> {
    STAGES.iter().copied().find(|candidate| *candidate == value)
}
/// Emit once from the actual bootstrap's first pre-UI failed terminal; no original text is rendered.
pub(super) fn report(cause: &NativeObservationError) {
    let record = Record::actual(cause);
    tracing::warn!(operation="child_transport_terminal",native_stage=%record.stage,native_win32=?record.win32);
}
/// Decode one complete fixed-vocabulary record inside the existing512-byte/first-failure slot.
pub(super) fn parse(bytes: &[u8]) -> Option<Record> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"child_transport_terminal\" ")?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let stage = stage(fields.next()?.strip_prefix("native_stage=")?)?;
    let value = fields.next()?.strip_prefix("native_win32=")?;
    let win32 = if value == "None" {
        None
    } else {
        let number = value.strip_prefix("Some(")?.strip_suffix(')')?;
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some(number.parse().ok()?)
    };
    if fields.next().is_some() {
        return None;
    }
    Some(Record { stage, win32 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        fmt, io,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
    #[derive(Clone)]
    struct Writer(Arc<Mutex<Vec<u8>>>);
    impl io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| io::Error::other("Diagnostic writer poisoned"))?
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn actual_formatter_transport_record_is_closed_and_bounded() -> Result<(), Box<dyn Error>> {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || Writer(output.clone()))
            .finish();
        let cause = NativeObservationError::new(
            "captured_parent_peer_mismatch",
            io::Error::from_raw_os_error(109),
        );
        tracing::subscriber::with_default(subscriber, || report(&cause));
        let bytes = bytes.lock().map_err(|_| "Diagnostic writer poisoned")?;
        let line = bytes.strip_suffix(b"\n").ok_or("Actual formatter newline missing")?;
        assert_eq!(
            parse(line),
            Some(Record { stage: "captured_parent_peer_mismatch", win32: Some(109) })
        );
        for line in [
            "operation=\"child_transport_terminal\" native_stage=unknown native_win32=None",
            "operation=\"child_transport_terminal\" native_stage=pipe_completion native_win32=Some(4294967296)",
            "operation=\"child_transport_terminal\" native_stage=pipe_completion native_win32=Some(-1)",
            "operation=\"child_transport_terminal\" native_stage=pipe_completion native_win32=None extra=secret",
            "operation=x operation=\"child_transport_terminal\" native_stage=pipe_completion native_win32=None",
        ] {
            assert_eq!(parse(line.as_bytes()), None);
        }
        Ok(())
    }
    struct Unknown(Arc<AtomicUsize>);
    impl fmt::Debug for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl fmt::Display for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl Error for Unknown {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.0.fetch_add(1, Ordering::SeqCst);
            None
        }
    }
    #[test]
    fn transport_projection_never_executes_unknown_error_callbacks_or_exports_its_label() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cause = NativeObservationError::new("private-path-token", Unknown(calls.clone()));
        assert_eq!(Record::actual(&cause), Record { stage: "unclassified", win32: None });
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
