// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish bounded ordinary records while the caller retains immutable native writer exclusion.
use super::*;
use crate::durable_storage::staging;

/// Stage complete bytes before same-parent create-only or checked replacement publication.
/// Replacement callers retain a separately pinned immutable anchor's native writer lock.
pub(super) fn write(
    parent: &Directory,
    leaf: &str,
    expected: Option<&RecordBytes>,
    bytes: &[u8],
    context: &JobContext,
) -> io::Result<()> {
    component(leaf)?;
    if bytes.len() > WORKER_RECORD_BYTES
        || expected.is_some_and(|old| old.as_slice().len() > WORKER_RECORD_BYTES)
    {
        return Err(invalid("Record publication exceeds its bound"));
    }
    check(context)?;
    match (parent.read(leaf), expected) {
        (Ok(mut original), Some(expected)) => {
            files::single_link(&original)?;
            if expected.leaf != leaf
                || expected.parent != parent.identity()?
                || expected.identity != files::file_identity(&original)?
                || files::read_bounded(&mut original, WORKER_RECORD_BYTES)? != expected.as_slice()
            {
                return Err(invalid("Record changed before publication"));
            }
        }
        (Ok(_), None) => return Err(io::ErrorKind::AlreadyExists.into()),
        (Err(cause), None) if cause.kind() == io::ErrorKind::NotFound => {}
        (Err(cause), _) => return Err(cause),
    }
    let staged = staging::unique_path(parent, ".record-stage")?;
    files::write_new(&staged, bytes)?;
    check(context)?;
    let destination = parent.path().join(leaf);
    if expected.is_some() {
        staging::replace_record(&staged, &destination)
    } else {
        staging::publish(&staged, &destination)
    }
}
