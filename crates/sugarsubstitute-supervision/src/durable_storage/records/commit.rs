// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded publication custody before a feature accepts irreversible security progress.
use crate::durable_storage::{
    check,
    files::{self, Directory},
    invalid,
    protection::{DomainFailure, Protection},
    staging,
};
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use sugarsubstitute_contracts::execution::JobContext;

/// One precreated protected record and its original parent/protection custody.
/// The feature retains writer exclusion and decides when accepted progress requires commitment.
/// Acquisition observes the original job controls. Completion performs no new cancellable open
/// and must run to its actual native outcome before the admitted worker can retire.
pub(crate) struct PreparedCommit<'a> {
    parent: &'a Directory,
    target: &'a str,
    staged: PathBuf,
    file: File,
    maximum: usize,
    protection: &'a mut Protection,
}

impl<'a> PreparedCommit<'a> {
    /// Precreate one protected stage while the original job still permits acquisition.
    /// `maximum` bounds caller-owned encoded bytes. The caller retains its original workspace
    /// claim for that allocation; the current job funds native custody and supplies controls.
    /// The target and stage are distinct flat siblings; the caller establishes target ownership.
    pub(crate) fn new(
        parent: &'a Directory,
        target: &'a str,
        prefix: &str,
        maximum: usize,
        protection: &'a mut Protection,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        files::flat_leaf(target)?;
        if maximum == 0 {
            return Err(invalid("Prepared record requires a nonzero byte bound"));
        }
        let staged = staging::unique_path(parent, prefix)?;
        let file = protection.file(&staged, context)?;
        Ok(Self { parent, target, staged, file, maximum, protection })
    }

    /// Commit complete encoded bytes even if job controls ended after preparation.
    /// The feature must check its original controls before returning authority to its caller.
    /// # Errors
    /// Rejects oversized/empty bytes, native writes, flushes, checked closure and publication.
    /// Failed staging remains for feature-owned recovery; existing target bytes stay authoritative.
    pub(crate) fn commit(self, bytes: &[u8]) -> io::Result<()> {
        let Self { parent, target, staged, mut file, maximum, protection } = self;
        let written = if bytes.is_empty() || bytes.len() > maximum {
            Err(invalid("Prepared record bytes exceed their admitted bound"))
        } else {
            file.write_all(bytes).and_then(|()| file.sync_all())
        };
        let closed = protection.close_file(file);
        match (written, closed) {
            (Ok(()), Ok(())) => staging::replace_record(&staged, &parent.path().join(target)),
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
            (Err(operation), Err(cause)) => {
                Err(io::Error::other(DomainFailure::Native { cause, operation: Some(operation) }))
            }
        }
    }
}

#[cfg(test)]
mod tests;
