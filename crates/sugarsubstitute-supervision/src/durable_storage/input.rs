// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read caller-selected preparation inputs within their original whole-job reservation.
//!
//! Read-only inputs follow the selected path, including supported filesystem links.
//! They grant no publication or retirement authority. Features supply their byte bound
//! and retain parsing, schema, precedence and preservation decisions. Original execution
//! controls remain observable after every returned open/read outcome, including failure.
mod error;
use super::record_read::{self, Allocation, InterruptedReads, ReadFailure};
pub use error::PreparationFileError;
use std::{fs::File, path::Path};
use sugarsubstitute_execution::EntryPreparationContext;

/// Read at most `maximum` bytes from one selected input on the admitted preparation worker.
/// The fixed allocation reserves one sentinel byte and stays within the original job's
/// whole reservation. Returned bytes remain part of that preparation's owned outcome.
/// File handles stay private and close before this function returns. This performs
/// synchronous I/O and belongs on the original finite worker, outside the UI thread.
/// # Errors
/// Preserves original open/read causes and mandatory later control failures separately.
/// Rejects expired/cancelled preparation, bounds exceeding its original reservation,
/// allocation failure, or input growing beyond `maximum`. Error formatting omits paths.
///
/// ```compile_fail
/// use std::path::Path;
/// use sugarsubstitute_contracts::execution::JobContext;
/// use sugarsubstitute_supervision::read_preparation_file;
/// fn read_without_original_preparation(context: &JobContext, path: &Path) {
///     let _result = read_preparation_file(context, path, 65_536);
/// }
/// ```
pub fn read_preparation_file(
    context: &EntryPreparationContext,
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, PreparationFileError> {
    context.check().map_err(PreparationFileError::Control)?;
    let capacity = maximum.checked_add(1).ok_or(PreparationFileError::TooLarge)?;
    if u64::try_from(capacity)
        .ok()
        .is_none_or(|bytes| bytes > context.job_context().memory().bytes())
    {
        return Err(PreparationFileError::TooLarge);
    }
    let file = record_read::observe(File::open(path), || context.observe_control().map(|_| ()))?;
    record_read::bounded(
        file,
        maximum,
        Allocation::Exact,
        InterruptedReads::Propagate,
        || context.check().map(|_| ()),
        || context.observe_control().map(|_| ()),
    )
    .map_err(PreparationFileError::from)
}

impl From<ReadFailure<sugarsubstitute_execution::EntryPreparationError>> for PreparationFileError {
    fn from(failure: ReadFailure<sugarsubstitute_execution::EntryPreparationError>) -> Self {
        match failure {
            ReadFailure::Control(cause) => Self::Control(cause),
            ReadFailure::Read { source, after } => Self::Read { source, after },
            ReadFailure::Capacity(cause) => Self::Capacity(cause),
            ReadFailure::TooLarge => Self::TooLarge,
        }
    }
}
