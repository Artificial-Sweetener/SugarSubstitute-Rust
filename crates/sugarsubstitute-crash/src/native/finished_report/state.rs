// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain every actual token before result interpretation and describe its Rust destination.
use super::{
    abi,
    control::ControlHistory,
    observation::{Failures, InvalidResult},
};
use sugarsubstitute_contracts::execution::{ExecutionId, JobId, Moment, ScopeId};

pub(super) struct Original {
    pub execution: ExecutionId,
    pub scope: ScopeId,
    pub job: JobId,
    pub deadline: Moment,
    pub counter: abi::Deadline,
}
/// Data/control allocations are destroyed before the outer actual token allocation guard.
/// Unexpected token output never replaces the original observed token.
pub(super) struct State {
    pub api: abi::Api,
    pub token: usize,
    pub unexpected: Option<usize>,
    pub history: ControlHistory,
    pub original: Original,
    pub cleanup: Option<Original>,
    pub facts: Option<abi::Facts>,
    pub preparation: Option<abi::Preparation>,
    pub failures: Failures,
}
impl State {
    /// Install raw native output custody before validating even its ABI version.
    /// Only validated actual retirement permits the caller to release its token guard.
    pub(super) fn observe(
        &mut self,
        result: &abi::Output,
        control: Option<super::control::ControlCause>,
    ) -> Result<(), InvalidResult> {
        let mut foreign = false;
        if result.lease != 0 {
            if self.token == 0 {
                self.token = result.lease
            } else if self.token != result.lease {
                self.unexpected.get_or_insert(result.lease);
                foreign = true;
            }
        }
        let checked = self.failures.observe(result, control);
        if foreign {
            self.failures.retain(super::observation::Failure::Invalid(InvalidResult));
            return Err(InvalidResult);
        }
        checked?;
        if result.facts_valid == 1 {
            self.facts = Some(result.facts)
        }
        Ok(())
    }
}
/// Fixed source-owned Rust retained cells and conservative synchronous ABI temporaries.
/// Opaque provider payload allocations retain their separately admitted source owners.
pub(super) const RUST_DESTINATION_BYTES: usize = 16 * 1024;
pub(super) const RUST_TEMPORARY_BYTES: usize = size_of::<abi::Open<'static>>()
    + size_of::<abi::Read<'static>>()
    + size_of::<abi::Call<'static>>()
    + size_of::<abi::Prepare<'static>>()
    + size_of::<abi::PrepareOutput>()
    + size_of::<abi::Disposition>()
    + size_of::<abi::DispositionOutput>()
    + size_of::<abi::Recover<'static>>()
    + size_of::<super::control::ControlInvocation<'static>>()
    + size_of::<sugarsubstitute_supervision::diagnostic_storage::ProtectedCaptureReadGuard<'static>>(
    );
const _: () = assert!(
    size_of::<State>() + RUST_TEMPORARY_BYTES + 2 * size_of::<usize>() < RUST_DESTINATION_BYTES
);
#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
