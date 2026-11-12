// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete feature-validated immutable records without occupying a final name partially.
use super::{
    check,
    files::{self, Directory},
    invalid, protection, staging,
};
use std::io::{self, Write};
use sugarsubstitute_contracts::execution::JobContext;

/// Flush and observe close before atomically publishing one new protected record.
/// Features validate their own codec and supply its byte bound, at most one MiB. Their admitted
/// workspace retains encoded bytes through this call. Unknown existing records are never replaced;
/// interrupted unique siblings remain intact and unreferenced for the feature's recovery policy.
/// # Errors
/// Preserves operation/control/close/publication causes, including both write and close failures.
pub(crate) fn publish(
    parent: &Directory,
    name: &str,
    bytes: &[u8],
    limit: usize,
    context: &JobContext,
) -> io::Result<()> {
    files::flat_leaf(name)?;
    if limit > 1024 * 1024 || bytes.len() > limit {
        return Err(invalid("Immutable record exceeds admission"));
    }
    let staged = staging::unique_path(parent, ".import-record")?;
    protection::with(context, |owner| {
        let mut file = owner.file(&staged, context)?;
        let written = (|| {
            check(context)?;
            file.write_all(bytes)?;
            files::sync(&file, context)?;
            files::file_identity(&file)
        })();
        let closed = owner.close_file(file);
        let identity = match (written, closed) {
            (Ok(identity), Ok(())) => identity,
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => return Err(cause),
            (Err(operation), Err(cause)) => {
                return Err(io::Error::other(protection::DomainFailure::Native {
                    cause,
                    operation: Some(operation),
                }));
            }
        };
        owner.publish(
            &staged,
            &parent.path().join(name),
            identity,
            protection::Publication::File { replace: false },
            context,
        )
    })
}
