// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select closed native destinations under the original storage operation.
use super::*;
use crate::native_claims::files::{NativeLayoutFailure, NativeRoleLayout};
use crate::protected_storage::{
    ProtectedStorageRoot, account,
    native_file::NativeFile,
    native_name::{NativeName, NativeSid},
    paths,
    record_file::StreamingPort,
    security,
};

/// Retain the sole original allocation ledger without exposing its claim.
#[derive(Clone)]
pub(crate) struct DiagnosticNativeGrant {
    original: DiagnosticAllocationOwner,
}

/// Retain one source-owned native allocation after its actual data/control drops.
pub(crate) struct DiagnosticNativeOwnerLoan {
    _allocation: DiagnosticAllocationLoan,
}
/// Retain a complete checked native path destination.
pub(crate) struct DiagnosticNativeNameLoan {
    _allocation: DiagnosticAllocationLoan,
}
/// Retain one finite native scratch session, including its nested phases.
pub(crate) struct DiagnosticNativeScratchLoan {
    role: NativeRole,
    allocation: DiagnosticAllocationLoan,
}
/// Retain a complete native directory scan generation.
pub(crate) struct DiagnosticNativeScanLoan {
    _allocation: DiagnosticAllocationLoan,
}
/// Retain both native scan vectors through their actual final data/control drops.
pub(crate) struct DiagnosticNativeScanRetention {
    credit: Option<std::sync::Arc<NativeScanCredit>>,
    original: DiagnosticNativeGrant,
}
struct NativeScanCredit {
    _allocation: DiagnosticNativeScanLoan,
}
#[repr(C)]
struct NativeScanArcStorage {
    strong: std::sync::atomic::AtomicUsize,
    weak: std::sync::atomic::AtomicUsize,
    data: NativeScanCredit,
}
impl DiagnosticNativeScanRetention {
    /// Describe this actual pinned shared control; the value grants no capacity.
    pub(crate) const fn control_bytes() -> usize {
        size_of::<NativeScanArcStorage>()
    }
}
impl Clone for DiagnosticNativeScanRetention {
    fn clone(&self) -> Self {
        Self { credit: self.credit.clone(), original: self.original.clone() }
    }
}
impl Drop for DiagnosticNativeScanRetention {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take()
            && let Some(credit) = std::sync::Arc::into_inner(credit)
        {
            drop(credit);
        }
    }
}
/// Retain the actual account SID bytes and their control.
pub(crate) struct DiagnosticNativeSidLoan {
    _allocation: DiagnosticAllocationLoan,
}
/// Retain an issued native failure destination before its external effect.
pub(crate) struct DiagnosticNativeCauseLoan {
    _allocation: DiagnosticAllocationLoan,
}

#[derive(Clone, Copy)]
pub(super) enum NativeRole {
    RootOwner,
    StreamingOwner,
    NativeFileOwner,
    NamespaceControl,
    OwnedName,
    PathScratch,
    AccountScratch,
    AccountSid,
    ScanState,
    ScanPage,
    NativeHash,
    NativeCause,
    NativeSecondaryCause,
}
impl NativeRole {
    pub(super) fn layout(self) -> Result<NativeRoleLayout, NativeLayoutFailure> {
        match self {
            Self::RootOwner => ProtectedStorageRoot::diagnostic_owner_layout(),
            Self::StreamingOwner => StreamingPort::diagnostic_owner_layout(),
            Self::NativeFileOwner => NativeFile::diagnostic_owner_layout(),
            Self::NamespaceControl => NativeName::diagnostic_control_layout(),
            Self::OwnedName => NativeName::diagnostic_owned_layout(),
            Self::PathScratch => Self::with_nested_security(paths::diagnostic_path_layout()?),
            Self::AccountScratch => account::diagnostic_account_layout(),
            Self::AccountSid => NativeSid::diagnostic_layout(),
            Self::ScanState => StreamingPort::diagnostic_scan_state_layout(),
            Self::ScanPage => {
                Self::with_nested_security(StreamingPort::diagnostic_scan_page_layout()?)
            }
            Self::NativeHash => NativeFile::diagnostic_hash_layout(),
            Self::NativeCause => ClaimError::diagnostic_native_cause_layout(),
            Self::NativeSecondaryCause => ClaimError::diagnostic_secondary_cause_layout(),
        }
    }
    fn with_nested_security(
        layout: NativeRoleLayout,
    ) -> Result<NativeRoleLayout, NativeLayoutFailure> {
        let nested = security::diagnostic_security_layout()?;
        layout
            .peak_bytes()
            .checked_sub(nested.peak_bytes())
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(layout)
    }
    pub(super) const fn is_scratch(self) -> bool {
        matches!(self, Self::PathScratch | Self::AccountScratch | Self::ScanPage | Self::NativeHash)
    }
}

impl DiagnosticAllocationOwner {
    pub(in crate::diagnostic_storage) fn native_grant(&self) -> DiagnosticNativeGrant {
        DiagnosticNativeGrant { original: self.clone() }
    }
}
impl DiagnosticNativeGrant {
    pub(in crate::diagnostic_storage) fn matches_allocation(
        &self,
        owner: &DiagnosticAllocationOwner,
    ) -> bool {
        self.original.same(owner)
    }
    /// Compare the actual original ledger; equality grants no native permission.
    pub(crate) fn matches_original(&self, other: &Self) -> bool {
        self.original.same(&other.original)
    }

    /// Reject an invalid native allocation state without a new cause allocation.
    /// This observes the actual logical operation and grants no native permission.
    pub(crate) fn reject_native_state(&self) -> DiagnosticNativeAllocationFailure {
        self.original.native_rejection()
    }

    /// Revalidate original allocation control without granting native permission.
    pub(crate) fn check(
        &self,
        current: &JobContext,
    ) -> Result<(), DiagnosticNativeAllocationFailure> {
        self.original.native_check(current)
    }
    /// Admit the actual root Box and complete fixed handle cells before allocation.
    pub(crate) fn root_owner(
        &self,
    ) -> Result<DiagnosticNativeOwnerLoan, DiagnosticNativeAllocationFailure> {
        self.metadata(NativeRole::RootOwner)
    }
    /// Admit the actual stream Box and its sole native-file cells before allocation.
    pub(crate) fn streaming_owner(
        &self,
    ) -> Result<DiagnosticNativeOwnerLoan, DiagnosticNativeAllocationFailure> {
        self.metadata(NativeRole::StreamingOwner)
    }
    /// Admit the native-file fields Box before constructing its complete capsule.
    pub(crate) fn native_file_owner(
        &self,
    ) -> Result<DiagnosticNativeOwnerLoan, DiagnosticNativeAllocationFailure> {
        self.metadata(NativeRole::NativeFileOwner)
    }
    /// Admit the actual shared name control before aliasable path construction.
    pub(crate) fn namespace_control(
        &self,
    ) -> Result<DiagnosticNativeOwnerLoan, DiagnosticNativeAllocationFailure> {
        self.metadata(NativeRole::NamespaceControl)
    }
    /// Admit actual path capacity under the current original operation.
    pub(crate) fn owned_name(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeNameLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeNameLoan { _allocation: self.current(NativeRole::OwnedName, context)? })
    }
    /// Admit the closed canonical-path peak before conversion.
    pub(crate) fn path_scratch(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScratchLoan, DiagnosticNativeAllocationFailure> {
        self.scratch(NativeRole::PathScratch, context)
    }
    /// Admit the finite account query peak before requesting its native buffer.
    pub(crate) fn account_scratch(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScratchLoan, DiagnosticNativeAllocationFailure> {
        self.scratch(NativeRole::AccountScratch, context)
    }
    /// Admit the actual retained SID destination and control before copying.
    pub(crate) fn account_sid(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeSidLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeSidLoan { _allocation: self.current(NativeRole::AccountSid, context)? })
    }
    /// Admit each native scan generation independently before replacing its predecessor.
    pub(crate) fn scan_retention(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScanRetention, DiagnosticNativeAllocationFailure> {
        let allocation =
            DiagnosticNativeScanLoan { _allocation: self.current(NativeRole::ScanState, context)? };
        Ok(DiagnosticNativeScanRetention {
            credit: Some(std::sync::Arc::new(NativeScanCredit { _allocation: allocation })),
            original: self.clone(),
        })
    }
    /// Admit a page plus recognized leaf and nested canonical validation peak.
    pub(crate) fn scan_page(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScratchLoan, DiagnosticNativeAllocationFailure> {
        self.scratch(NativeRole::ScanPage, context)
    }
    /// Admit the concrete hash buffer before reading actual file bytes.
    pub(crate) fn native_hash(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScratchLoan, DiagnosticNativeAllocationFailure> {
        self.scratch(NativeRole::NativeHash, context)
    }
    /// Admit one concrete native first-cause destination before the external effect.
    pub(crate) fn native_cause(
        &self,
    ) -> Result<DiagnosticNativeCauseLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeCauseLoan {
            _allocation: self.original.native_metadata(NativeRole::NativeCause)?,
        })
    }
    /// Admit one concrete secondary-cause destination before checked cleanup.
    pub(crate) fn native_secondary_cause(
        &self,
    ) -> Result<DiagnosticNativeCauseLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeCauseLoan {
            _allocation: self.original.native_metadata(NativeRole::NativeSecondaryCause)?,
        })
    }
    fn metadata(
        &self,
        role: NativeRole,
    ) -> Result<DiagnosticNativeOwnerLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeOwnerLoan { _allocation: self.original.native_metadata(role)? })
    }
    fn current(
        &self,
        role: NativeRole,
        context: &JobContext,
    ) -> Result<DiagnosticAllocationLoan, DiagnosticNativeAllocationFailure> {
        self.original.native_current(role, context)
    }
    fn scratch(
        &self,
        role: NativeRole,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScratchLoan, DiagnosticNativeAllocationFailure> {
        Ok(DiagnosticNativeScratchLoan { role, allocation: self.current(role, context)? })
    }
}

/// Borrow the already-admitted path peak without a second scratch admission.
pub(crate) struct DiagnosticNativePathPhase<'a> {
    scratch: &'a mut DiagnosticNativeScratchLoan,
}
/// Borrow the already-admitted security peak without a second scratch admission.
pub(crate) struct DiagnosticNativeSecurityPhase<'a> {
    scratch: &'a mut DiagnosticNativeScratchLoan,
}
impl DiagnosticNativeScratchLoan {
    /// Check only the admitted account query role before its finite native buffer.
    /// Matching the role grants no account or native permission.
    pub(crate) fn check_account(
        &self,
        current: &JobContext,
    ) -> Result<(), DiagnosticNativeAllocationFailure> {
        if !matches!(self.role, NativeRole::AccountScratch) {
            return Err(self.allocation.owner.native_rejection());
        }
        self.allocation.owner.native_check(current)
    }

    /// Borrow path conversion only from a role whose closed peak includes it.
    pub(crate) fn borrow_path(
        &mut self,
    ) -> Result<DiagnosticNativePathPhase<'_>, DiagnosticNativeAllocationFailure> {
        if !matches!(self.role, NativeRole::PathScratch | NativeRole::ScanPage) {
            return Err(DiagnosticNativeAllocationFailure::rejected(None, Code::Admission));
        }
        Ok(DiagnosticNativePathPhase { scratch: self })
    }
    /// Borrow security only from a role whose closed peak includes its overlap.
    pub(crate) fn borrow_security(
        &mut self,
    ) -> Result<DiagnosticNativeSecurityPhase<'_>, DiagnosticNativeAllocationFailure> {
        if !matches!(self.role, NativeRole::PathScratch | NativeRole::ScanPage) {
            return Err(DiagnosticNativeAllocationFailure::rejected(None, Code::Admission));
        }
        Ok(DiagnosticNativeSecurityPhase { scratch: self })
    }
}
impl DiagnosticNativePathPhase<'_> {
    /// Observe and revalidate original and live current control outside ledger locks.
    pub(crate) fn check(
        &self,
        current: &JobContext,
    ) -> Result<(), DiagnosticNativeAllocationFailure> {
        self.scratch.allocation.owner.native_check(current)
    }
}
impl DiagnosticNativeSecurityPhase<'_> {
    /// Observe and revalidate original and live current control outside ledger locks.
    pub(crate) fn check(
        &self,
        current: &JobContext,
    ) -> Result<(), DiagnosticNativeAllocationFailure> {
        self.scratch.allocation.owner.native_check(current)
    }
}

/// Return closed allocation rejection inline, without allocating a native cause.
#[derive(Clone, Debug)]
pub(crate) struct DiagnosticNativeAllocationFailure {
    operation: Option<LogicalStoreOperationKey>,
    code: Code,
    after: Option<Code>,
    pub(crate) storage_code: Code,
    pub(crate) storage_after: Option<Code>,
}
impl DiagnosticNativeAllocationFailure {
    pub(in crate::diagnostic_storage) const fn rejected(
        operation: Option<LogicalStoreOperationKey>,
        code: Code,
    ) -> Self {
        Self { operation, code, after: None, storage_code: Code::Admission, storage_after: None }
    }
    pub(crate) const fn operation(&self) -> Option<LogicalStoreOperationKey> {
        self.operation
    }
    pub(crate) const fn code(&self) -> Code {
        self.code
    }
    pub(crate) const fn post_control_failure(&self) -> Option<Code> {
        self.after
    }
    pub(super) fn with_post_control(mut self, after: Option<Self>) -> Self {
        self.after = after.map(|failure| failure.code);
        self
    }
}
impl std::fmt::Display for DiagnosticNativeAllocationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Diagnostic native allocation failed: {:?}", self.code)
    }
}
impl std::error::Error for DiagnosticNativeAllocationFailure {}
