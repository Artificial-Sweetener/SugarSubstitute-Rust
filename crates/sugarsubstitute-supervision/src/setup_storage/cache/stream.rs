// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish admitted streams through declared scratch files without exposing writable handles.
use super::*;
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
    let staged = staging::unique_path(&parent, "cache-pending")?;
    let staged_name = staged
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Cache staging leaf is absent"))?
        .to_owned();
    lease.declare_owned([staged_name.clone()], context)?;
    protection::with(context, |protection| {
        let mut file = protection.file(&staged, context)?;
        let mut output = Bounded { file: &mut file, remaining: length, context };
        write(&mut output)?;
        if output.remaining != 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Cache stream is shorter than declared",
            ));
        }
        check(context)?;
        file.sync_all()?;
        let identity = files::file_identity(&file)?;
        drop(file);
        protection.publish(
            &staged,
            &parent.path().join(name),
            identity,
            protection::Publication::File { replace: true },
            context,
        )
    })?;
    let mut next = lease.record.clone();
    next.files.retain(|name| name != &staged_name);
    record::save(&lease.directory, &next, context)?;
    lease.record = next;
    Ok(())
}
/// Grant only exact remaining sequential writes; buffering and verification belong to the feature.
struct Bounded<'a> {
    file: &'a mut File,
    remaining: u64,
    context: &'a JobContext,
}
impl Write for Bounded<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        check(self.context)?;
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cache stream exceeds its declared length",
            ));
        }
        let count = self.file.write(bytes)?;
        self.remaining -= count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        check(self.context)?;
        self.file.flush()
    }
}
