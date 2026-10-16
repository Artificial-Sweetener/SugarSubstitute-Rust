// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain each actual native observation vector and its original capacity through final aliases.
use super::*;

struct DiagnosticNativeScanLoan {
    _allocation: DiagnosticAllocationLoan,
}
/// Retain the admitted concrete capacity until the last shared scan owner is destroyed.
pub(crate) struct DiagnosticNativeScanRetention {
    credit: Option<std::sync::Arc<NativeScanCredit>>,
    capacity: usize,
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
    /// Require the actual original and exact vector capacity before replacing ownership.
    pub(crate) fn matches(&self, original: &DiagnosticNativeGrant, capacity: usize) -> bool {
        self.credit.is_some()
            && self.capacity == capacity
            && self.original.matches_original(original)
    }
}
impl Clone for DiagnosticNativeScanRetention {
    fn clone(&self) -> Self {
        Self {
            credit: self.credit.clone(),
            capacity: self.capacity,
            original: self.original.clone(),
        }
    }
}
impl Drop for DiagnosticNativeScanRetention {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take() {
            drop(std::sync::Arc::into_inner(credit));
        }
    }
}
impl DiagnosticNativeGrant {
    /// Admit one complete bounded observation vector and its concrete shared control.
    /// Replacement admission coexists with the old vector's actual retained capacity.
    /// # Errors
    /// Rejects zero/over-bound capacities, closed or foreign original controls,
    /// and unavailable cumulative residual before any vector allocation occurs.
    pub(crate) fn scan_retention(
        &self,
        capacity: usize,
        context: &JobContext,
    ) -> Result<DiagnosticNativeScanRetention, DiagnosticNativeAllocationFailure> {
        let allocation = DiagnosticNativeScanLoan {
            _allocation: self.current(NativeRole::ScanState(capacity), context)?,
        };
        Ok(DiagnosticNativeScanRetention {
            credit: Some(std::sync::Arc::new(NativeScanCredit { _allocation: allocation })),
            capacity,
            original: self.clone(),
        })
    }
}
