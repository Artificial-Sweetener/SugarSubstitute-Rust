// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed cross-crate envelope for the actual recovery-origin commit and checked read.
use super::*;
use crate::diagnostic_storage::CheckedRecoveryCaptureOrigin;
use sugarsubstitute_contracts::{
    crash::RecoveryCaptureIdentity, incident::BuildIdentity, lifecycle::CoordinatorKey,
};
#[repr(C)]
struct OriginRecord {
    operation: LogicalStoreOperationKey,
    origin: RecoveryCaptureIdentity,
    root_identity: [u8; 24],
    capture_key: CoordinatorKey,
    build: BuildIdentity,
}
#[repr(C)]
struct OriginCommit {
    record: OriginRecord,
    seed: [usize; DIAGNOSTIC_SEED_RESULT_INLINE_BYTES.div_ceil(size_of::<usize>())],
    activation: Option<DiagnosticActivation>,
    checked: Option<CheckedRecoveryCaptureOrigin>,
    custody: DiagnosticRecoveryCaptureOriginLoan,
}
/// Compiled commit cells plus bounded build strings for record, checked read and one producer temporary.
/// All fit the existing8MiB diagnostic owner; no new reservation is introduced.
pub const DIAGNOSTIC_CAPTURE_ORIGIN_COMMIT_INLINE_BYTES: usize =
    size_of::<OriginCommit>() + 3 * BuildIdentity::MAX_BYTES;
/// Exact compiled upper envelope for one checked historical read, in bytes.
pub const DIAGNOSTIC_CAPTURE_ORIGIN_READ_INLINE_BYTES: usize =
    size_of::<CheckedRecoveryCaptureOrigin>() + BuildIdentity::MAX_BYTES;
