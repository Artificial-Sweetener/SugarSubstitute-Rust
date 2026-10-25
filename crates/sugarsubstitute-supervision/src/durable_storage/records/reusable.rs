// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish bounded records through schema-owned reusable stages and observed native closure.
use crate::durable_storage::{
    check,
    control::effect,
    files::{self, Directory, ExistingAccess},
    invalid, protection,
};
use std::io::{self, Write};
use sugarsubstitute_contracts::execution::JobContext;

/// Reuse only an authorized protected sibling stage, preserving the final record until publish.
/// The feature owns both names, byte limit and replacement decision. Caller-owned source bytes
/// remain funded by its admitted workspace. Stage validation precedes any truncation.
pub(crate) fn publish_reusable(
    parent: &Directory,
    target: &str,
    stage: &str,
    bytes: &[u8],
    maximum: usize,
    replace: bool,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    files::flat_leaf(target)?;
    files::flat_leaf(stage)?;
    if target.eq_ignore_ascii_case(stage)
        || bytes.len() > maximum
        || u64::try_from(maximum).ok().is_none_or(|bound| bound > context.memory().bytes())
    {
        return Err(invalid("Reusable record stage aliases its target or exceeds its bound"));
    }
    let staged = parent.path().join(stage);
    protection::with(context, |protection| {
        let mut file = match parent.open_existing(stage, ExistingAccess::Exclusive, context) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                protection.file(&staged, context)?
            }
            Err(cause) => return Err(cause),
        };
        let written = effect(context, || {
            protection.verify_file(&file)?;
            files::single_link(&file)?;
            check(context)?;
            file.set_len(0)?;
            check(context)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            check(context)?;
            files::file_identity(&file)
        });
        let closed = protection.close_file(file);
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
        protection.publish(
            &staged,
            &parent.path().join(target),
            identity,
            protection::Publication::File { replace },
            context,
        )
    })
}

#[cfg(test)]
mod tests;
