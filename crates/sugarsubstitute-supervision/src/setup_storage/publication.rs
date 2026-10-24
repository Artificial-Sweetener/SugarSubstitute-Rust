// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Journal exact file identities before making temporary publication inputs durable.
mod journal;
#[cfg(test)]
mod tests;
use super::{check, invalid};
use crate::durable_storage::{
    files::{self, Directory},
    protection::{Protection, Publication},
    staging,
};
use journal::Record;
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};
use sugarsubstitute_contracts::execution::JobContext;

/// An unpublished file and its flushed ownership record under the caller's mutation exclusion.
/// Dropping the value closes its handles; recovery consumes the persisted identity journal.
pub(super) struct Stage<'a> {
    parent: &'a Directory,
    record: Record,
    journal_identity: (u32, u32, u32),
    file: File,
}
impl<'a> Stage<'a> {
    /// Persist complete ownership before allowing a partial payload to survive process exit.
    pub(super) fn create(
        parent: &'a Directory,
        protection: &mut Protection,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let id = staging::identity()?;
        let source = parent.path().join(Record::stage_name(&id));
        let file = protection.transient_file(&source, context)?;
        let record = Record::new(id, parent.identity()?, files::file_identity(&file)?);
        let journal_path = parent.path().join(record.journal_name());
        let mut journal = protection.transient_file(&journal_path, context)?;
        journal.write_all(&serde_json::to_vec(&record).map_err(io::Error::other)?)?;
        files::sync(&journal, context)?;
        let journal_identity = files::file_identity(&journal)?;
        protection.retain_transient(&journal, context)?;
        protection.close_file(journal)?;
        protection.retain_transient(&file, context)?;
        Ok(Self { parent, record, journal_identity, file })
    }

    /// Borrow the original owned writer; callers verify complete bytes before publication.
    pub(super) fn file(&mut self) -> &mut File {
        &mut self.file
    }

    /// Publish complete bytes atomically, then retire only the matching ownership journal.
    /// The caller verifies any replaced destination under its continuously held writer exclusion.
    /// Once publication commits, deferred journal cleanup is observable without undoing success.
    pub(super) fn publish(
        self,
        destination: &Path,
        replace: bool,
        protection: &mut Protection,
        context: &JobContext,
    ) -> io::Result<()> {
        if destination.parent() != Some(self.parent.path()) {
            return Err(invalid("Journaled publication requires a sibling destination"));
        }
        files::sync(&self.file, context)?;
        protection.close_file(self.file)?;
        protection.publish(
            &self.parent.path().join(self.record.stage()),
            destination,
            self.record.file,
            Publication::File { replace },
            context,
        )?;
        if let Err(cause) = protection.remove_journaled(
            &self.parent.path().join(self.record.journal_name()),
            self.journal_identity,
            context,
        ) {
            tracing::warn!(operation = "publication_cleanup_preserved", attempt = self.record.id(),
                error = %cause, "Published file retained; journal cleanup deferred");
        }
        Ok(())
    }
}

/// Retire bounded, recognized publication attempts under the same exclusion used to create them.
/// Foreign/corrupt records and changed objects remain intact and observable. Recovery never
/// follows a journal to a published destination or to an authoritative user-data filename.
pub(super) fn recover(
    parent: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let mut seen = 0;
    for entry in parent.entries(context)? {
        check(context)?;
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(id) = Record::journal_id(name) else {
            continue;
        };
        seen += 1;
        if seen > 256 {
            tracing::warn!(
                operation = "publication_cleanup_deferred",
                limit = 256,
                "Additional publication records remain for a later maintenance pass"
            );
            break;
        }
        if let Err(cause) = recover_one(parent, name, id, protection, context) {
            check(context)?;
            tracing::warn!(operation = "publication_cleanup_preserved", attempt = id, error = %cause,
                "Publication recovery preserved an uncertain or unavailable object");
        }
    }
    Ok(())
}

/// Publish an immutable protected record, accepting only byte-identical existing output.
/// The caller holds mutation exclusion and runs recovery before creating this operation's files.
pub(super) fn write_immutable(
    parent: &Directory,
    leaf: &str,
    bytes: &[u8],
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    files::relative(leaf)?;
    match parent.read(leaf) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            return if files::read_bounded(&mut file, bytes.len())? == bytes {
                Ok(())
            } else {
                Err(invalid("Immutable protected record differs from the expected bytes"))
            };
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let mut stage = Stage::create(parent, protection, context)?;
    stage.file().write_all(bytes)?;
    stage.publish(&parent.path().join(leaf), false, protection, context)
}

/// Decode protected ownership before requesting deletion of either exact native object.
fn recover_one(
    parent: &Directory,
    name: &str,
    id: &str,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let (record, identity) = {
        let mut file = parent.read(name)?;
        protection.verify_file(&file)?;
        let record =
            Record::decode(&files::read_bounded(&mut file, 4096)?, id, parent.identity()?)?;
        (record, files::file_identity(&file)?)
    };
    protection.remove_journaled(&parent.path().join(record.stage()), record.file, context)?;
    protection.remove_journaled(&parent.path().join(name), identity, context)
}
