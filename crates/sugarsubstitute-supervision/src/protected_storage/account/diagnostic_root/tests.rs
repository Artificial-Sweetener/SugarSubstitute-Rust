// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual token/private-directory pins without changing account storage or global settings.
use super::*;
use crate::coordinator_entry::DiagnosticStorageLocation;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::execution::JobContext;
static NEXT: AtomicU64 = AtomicU64::new(1);
fn run(work: impl FnOnce(JobContext) -> Result<(), ClaimError> + Send + 'static) -> TestResult {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = 3 * 1024 * 1024;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, work)?;
    let done = rig.consume_job(&mut job)?;
    match done.outcome {
        JobOutcome::Success(()) => (),
        JobOutcome::Failure(cause) => return Err(cause.into()),
        other => return Err(format!("Diagnostic directory job failed: {other:?}").into()),
    }
    drop(done.memory);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
struct Native {
    parent: PathBuf,
    handles: Vec<crate::native_claims::files::Handle>,
    budget: crate::native_claims::files::Budget,
    resolved: account::ResolvedNamespace,
}
impl Native {
    fn new(context: &JobContext) -> Result<Self, ClaimError> {
        let parent = std::env::temp_dir().join(format!(
            "diagnostic-root-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&parent).map_err(|cause| ClaimError::new("test_directory", cause))?;
        let budget = crate::native_claims::files::Budget::Protected(Arc::new(
            context
                .memory()
                .claim(2 * 1024 * 1024)
                .map_err(|cause| ClaimError::new("test_claim", cause))?,
        ));
        let mut handles = Vec::with_capacity(260);
        let mut phase = native_phase::NativePhase::ordinary(&budget)?;
        let observed = account::resolve_current(
            &mut handles,
            budget.clone(),
            account::PrimaryAccountAccess::Query,
            &mut phase,
        )?;
        let resolved = account::ResolvedNamespace {
            namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
            account_sid: observed.sid.into_ordinary()?,
            session_id: observed.session_id,
            installation_identity: [6; 24],
            directory: PathBuf::from(r"C:\unopened-account-claims"),
            directory_slot: usize::MAX,
            directory_identity: [7; 24],
            diagnostics: None,
        };
        Ok(Self { parent, handles, budget, resolved })
    }
    fn location(&self) -> DiagnosticStorageLocation {
        DiagnosticStorageLocation::Explicit(self.parent.join("private"))
    }
    fn resolve(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        let location = self.location();
        account::resolve_diagnostic_root(
            &mut self.resolved,
            &location,
            &mut self.handles,
            self.budget.clone(),
            context,
        )
    }
    fn close(&mut self) -> Result<(), ClaimError> {
        crate::native_claims::files::close_all(&mut self.handles)
    }
    fn finish(mut self) -> Result<(), ClaimError> {
        self.close()?;
        std::fs::remove_dir_all(&self.parent)
            .map_err(|cause| ClaimError::new("test_directory_cleanup", cause))
    }
}
#[test]
fn explicit_native_root_pins_same_directory_for_writer_validation_and_durable_reopen() -> TestResult
{
    run(|context| {
        let mut native = Native::new(&context)?;
        let expected_namespace = native.resolved.namespace;
        let expected_sid = native.resolved.account_sid.clone();
        let expected_installation = native.resolved.installation_identity;
        native.resolve(&context)?;
        assert!(native.handles.len() <= HANDLE_BOUND);
        assert_eq!(native.handles.capacity(), HANDLE_BOUND);
        let retained = size_of::<account::ResolvedNamespace>()
            + native.handles.capacity() * size_of::<crate::native_claims::files::Handle>()
            + native.resolved.account_sid.capacity()
            + native.resolved.directory.capacity()
            + native.resolved.diagnostic_directory().0.as_os_str().len();
        let scratch = paths::diagnostic_path_layout()
            .map_err(|cause| ClaimError::new("test_path_layout", cause))?
            .peak;
        match &native.budget {
            crate::native_claims::files::Budget::Protected(original) => {
                assert_eq!(original.bytes(), crate::coordinator_entry::ENTRY_PREPARATION_BYTES);
                assert!(retained as u64 + scratch as u64 + 256 * 1024 <= original.bytes());
            }
            _ => return Err(ClaimError::invalid("test_original_budget")),
        }
        let (path, slot, identity) = native.resolved.diagnostic_directory();
        let path = path.to_owned();
        assert_ne!(slot, usize::MAX);
        assert_ne!(identity, [0; 24]);
        assert!(
            path.starts_with(
                std::fs::canonicalize(&native.parent)
                    .map_err(|cause| ClaimError::new("test_canonical", cause))?
            )
        );
        assert_eq!(native.resolved.namespace, expected_namespace);
        assert_eq!(native.resolved.account_sid, expected_sid);
        assert_eq!(native.resolved.installation_identity, expected_installation);
        let mut reader_root = ProtectedStorageRoot::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_reader_claim", cause))?,
        )?;
        reader_root.resolve(&path, &context)?;
        reader_root.validate_diagnostic_namespace(&native.resolved, &native.handles, &context)?;
        std::fs::write(path.join("preserved.payload"), b"original-payload")
            .map_err(|cause| ClaimError::new("test_payload", cause))?;
        reader_root.retire(&context)?;
        native.close()?;
        native.resolved.diagnostics = None;
        let different_case = DiagnosticStorageLocation::Explicit(native.parent.join("PRIVATE"));
        account::resolve_diagnostic_root(
            &mut native.resolved,
            &different_case,
            &mut native.handles,
            native.budget.clone(),
            &context,
        )?;
        assert_eq!(native.resolved.diagnostic_directory().0, path);
        assert_eq!(native.resolved.diagnostic_directory().2, identity);
        assert_eq!(
            std::fs::read(path.join("preserved.payload"))
                .map_err(|cause| ClaimError::new("test_payload", cause))?,
            b"original-payload"
        );
        let location = native.location();
        assert!(
            account::resolve_diagnostic_root(
                &mut native.resolved,
                &location,
                &mut Vec::new(),
                native.budget.clone(),
                &context
            )
            .is_err()
        );
        native.finish()
    })
}
#[test]
fn explicit_root_foreign_existing_protection_fails_without_repair_or_account_fallback() -> TestResult
{
    run(|context| {
        let mut native = Native::new(&context)?;
        let leaf = native.parent.join("private");
        std::fs::create_dir(&leaf).map_err(|cause| ClaimError::new("test_unprotected", cause))?;
        std::fs::write(leaf.join("preserve"), b"foreign")
            .map_err(|cause| ClaimError::new("test_foreign", cause))?;
        let rejected = native.resolve(&context).is_err();
        assert!(rejected);
        assert!(native.resolved.diagnostics.is_none());
        assert_eq!(
            native.resolved.diagnostic_directory().0,
            Path::new(r"C:\unopened-account-claims")
        );
        assert_eq!(
            std::fs::read(leaf.join("preserve"))
                .map_err(|cause| ClaimError::new("test_foreign", cause))?,
            b"foreign"
        );
        native.finish()
    })
}
#[test]
fn explicit_root_missing_parent_and_exhausted_original_handle_envelope_create_no_leaf() -> TestResult
{
    run(|context| {
        let mut native = Native::new(&context)?;
        let missing =
            DiagnosticStorageLocation::Explicit(native.parent.join("missing").join("private"));
        assert!(
            account::resolve_diagnostic_root(
                &mut native.resolved,
                &missing,
                &mut native.handles,
                native.budget.clone(),
                &context
            )
            .is_err()
        );
        assert!(!native.parent.join("missing").exists());
        native.close()?;
        native.handles = Vec::new();
        assert!(native.resolve(&context).is_err());
        assert!(!native.parent.join("private").exists());
        assert!(native.resolved.diagnostics.is_none());
        native.finish()
    })
}
