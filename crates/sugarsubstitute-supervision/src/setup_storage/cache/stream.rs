// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish admitted streams through declared scratch files without exposing writable handles.
use super::*;
use crate::durable_storage::streamed_record::StreamStage;
use std::io::Write;

/// Own one exact sequential publication with bounded workspace and crash-visible declarations.
pub(super) fn write(
    lease: &mut CacheLease,
    name: &str,
    length: u64,
    context: &JobContext,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
    lease.require_declared_writer()?;
    files::relative(name).map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
    if name.contains('/') {
        return Err(invalid("Atomic cache records require a flat leaf"));
    }
    let used = lease.inspect(context)?.bytes;
    if used.checked_add(length).is_none_or(|bytes| bytes > lease.record.reserved) {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "Cache entry byte reservation exhausted",
        ));
    }
    lease.declare_owned([name.to_owned()], context)?;
    let parent = Directory::open(&lease.path(), false)?;
    let stage = StreamStage::plan(&parent, name, length, context)?;
    let staged_name = stage.stage_name()?.to_owned();
    lease.declare_owned([staged_name.clone()], context)?;
    let mut stream = stage.create(context)?;
    stream.write_with(context, write)?;
    stream.commit(context)?;
    let mut next = lease.record.clone();
    next.files.retain(|name| name != &staged_name);
    record::save(&lease.directory, &next, context)?;
    lease.record = next;
    Ok(())
}
