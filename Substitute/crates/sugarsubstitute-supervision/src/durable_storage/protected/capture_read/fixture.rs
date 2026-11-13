// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create real protected synthetic source generations for external-boundary tests.
use super::*;
/// Own a genuinely created protected synthetic generation and its original claim.
/// This fixture never manufactures a generation/read guard or completed dump.
pub struct ProtectedCaptureSourceFixture {
    root: ProtectedStorageRoot,
    fixture: PathBuf,
    _original: Budget,
}
impl ProtectedCaptureSourceFixture {
    /// Observe the actual original source job reservation requirement in bytes.
    /// This value grants no admission or generation authority.
    pub const fn required_reservation_bytes() -> u64 {
        STORAGE_BYTES
    }
    /// Create only a new caller-owned fixture directory and actual protected child.
    /// Invoke on a genuine admitted original1MiB worker; configure all paths under
    /// the owned test artifact root before invocation. Existing paths are rejected.
    /// # Errors
    /// Preserves actual creation/identity/control/cleanup causes and native custody.
    pub fn create(
        context: &JobContext,
        owner: CoordinatorKey,
        fixture: PathBuf,
    ) -> Result<Self, ClaimError> {
        if fixture.capacity() > 65_536 {
            return Err(ClaimError::invalid("fixture_path_capacity"));
        }
        let root = ProtectedStorageRoot::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("fixture_claim", cause))?,
        )?;
        let budget =
            root.state.as_ref().ok_or_else(|| ClaimError::invalid("fixture_root"))?.budget.clone();
        super::super::check(context)?;
        let created = std::fs::create_dir(&fixture)
            .map_err(|cause| ClaimError::new("fixture_create_new", cause));
        ClaimError::finish(created, super::super::check(context))?;
        let mut source = Self { root, fixture, _original: budget };
        let prepared = (|| {
            source.create_protected(context)?;
            source.root.resolve(&source.fixture.join("protected"), context)?;
            source.root.create_capture_generation(owner, context)
        })();
        if let Err(cause) = prepared {
            return ClaimError::finish(Err(cause), source.root.retire(context));
        }
        Ok(source)
    }
    /// Borrow the actual retained root; only its existing producer mints read custody.
    pub fn root_mut(&mut self) -> &mut ProtectedStorageRoot {
        &mut self.root
    }
    /// Borrow the owned artifact location for machine-only evidence, never authority.
    pub fn directory(&self) -> &Path {
        &self.fixture
    }
    /// Checked-close the original native root after all actual generation leases retire.
    /// # Errors
    /// Busy/failed close retains original native handles and both source controls.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.root.retire(context)
    }
    /// Observe only actual checked native root retirement.
    pub fn is_retired(&self) -> bool {
        self.root.is_retired()
    }
    fn create_protected(&self, context: &JobContext) -> Result<(), ClaimError> {
        let budget = &self._original;
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(HANDLE_BOUND)
            .map_err(|cause| ClaimError::new("fixture_handles", cause))?;
        let mut phase = native_phase::NativePhase::ordinary(budget)?;
        let produced = (|| {
            phase.context(context)?;
            let account = account::resolve_current(
                &mut handles,
                budget.clone(),
                account::PrimaryAccountAccess::Query,
                &mut phase,
            )?;
            paths::create_directory(
                &self.fixture.join("protected"),
                account.sid.bytes()?,
                budget.clone(),
                &mut handles,
                &mut phase,
            )?;
            phase.context(context)
        })();
        let mut cleanup = Ok(());
        for handle in handles.iter_mut().rev() {
            cleanup = ClaimError::finish(cleanup, handle.close());
        }
        ClaimError::finish(produced, cleanup)
    }
}
