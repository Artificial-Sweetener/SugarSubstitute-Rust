// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake external primitives while the sole fixed-record owner retains all custody.
use super::*;
/// Perform actual external primitives; algorithm, authority and custody remain in core.
/// Every fallible output-producing method adopts into the supplied owner before
/// reporting failure. Callers retain the original execution and finite deadlines.
pub(crate) trait RecordBoundary<const N: usize> {
    fn verify_exclusion(&mut self, file: &HeldFile<N>) -> Result<(), ClaimError>;
    fn read_shape(
        &mut self,
        file: &HeldFile<N>,
        expected: Option<[u8; 24]>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
        operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<RecordRead<N>, ClaimError>;
    fn create_stage_into(
        &mut self,
        file: &HeldFile<N>,
        stage: &mut Stage<N>,
    ) -> Result<(), ClaimError>;
    fn reopen_stage_into(
        &mut self,
        file: &HeldFile<N>,
        stage: &mut Stage<N>,
    ) -> Result<(), ClaimError>;
    fn stage_is_open(&self, stage: &Stage<N>) -> bool;
    fn verify_stage(&mut self, file: &HeldFile<N>, stage: &mut Stage<N>) -> Result<(), ClaimError>;
    fn write_stage(
        &mut self,
        stage: &mut Stage<N>,
        encoded: &[u8; N],
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError>;
    fn activate_stage(
        &mut self,
        file: &HeldFile<N>,
        stage: &mut Stage<N>,
    ) -> Result<(), ClaimError>;
    fn read_activated(
        &mut self,
        stage: &mut Stage<N>,
        expected: &[u8; N],
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError>;
    fn flush_stage(&mut self, stage: &mut Stage<N>) -> Result<(), ClaimError>;
    fn close_stage(&mut self, stage: &mut Stage<N>) -> Result<(), ClaimError>;
    fn close_temporary(
        &mut self,
        file: &HeldFile<N>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
        operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<(), ClaimError>;
}
