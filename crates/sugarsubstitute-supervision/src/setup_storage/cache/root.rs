// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete cache scopes before exposing them and verify their private ownership marker.
use super::*;
use std::io::Write;

/// Publish a complete private cache scope; retained directory handles bind native operations.
pub(super) fn open(
    anchor: &Path,
    policy: CachePolicy,
    admitted: &sugarsubstitute_contracts::execution::MemoryLease,
    context: &JobContext,
) -> io::Result<RuntimeCache> {
    check(context)?;
    policy.validate()?;
    let memory = admitted.claim(ROOT_BYTES).map_err(io::Error::other)?;
    let parent = Directory::open(anchor, false)?;
    let expected = "SugarSubstitute runtime cache 1\n";
    let path = parent.path.join(DIRECTORY);
    match Directory::open(&path, false) {
        Ok(_) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            let staged = staging::unique_path(&parent, ".cache-initializing")?;
            let identity = protection::with(context, |protection| {
                protection.directory(&staged, Mode::CreateNew, context)?;
                let directory = Directory::open(&staged, false)?;
                let mut owner = protection.file(&staged.join("owner"), context)?;
                owner.write_all(expected.as_bytes())?;
                owner.sync_all()?;
                protection.file(&staged.join("writer"), context)?.sync_all()?;
                for namespace in Namespace::ALL {
                    protection.directory(
                        &staged.join(namespace.directory()),
                        Mode::CreateNew,
                        context,
                    )?;
                }
                directory.identity()
            })?;
            let published = protection::with(context, |protection| {
                protection.publish(
                    &staged,
                    &path,
                    identity,
                    protection::Publication::Directory,
                    context,
                )
            });
            if let Err(cause) = published
                && !record::collision(&cause)
            {
                return Err(cause);
            }
        }
        Err(cause) => return Err(cause),
    }
    protection::with(context, |protection| {
        protection.directory(&path, Mode::Existing, context)?;
        let root = Directory::open(&path, false)?;
        let mut owner = root.read("owner")?;
        protection.verify_file(&owner)?;
        if files::read_bounded(&mut owner, 256)? != expected.as_bytes() {
            return Err(invalid("Cache scope ownership marker is invalid"));
        }
        Ok(RuntimeCache {
            scope: std::sync::Arc::new(Scope {
                anchor: parent,
                root,
                _owner: owner,
                policy,
                memory: admitted.clone(),
                _memory: memory,
            }),
        })
    })
}

impl RuntimeCache {
    /// Serialize admission and cleanup across processes without blocking a storage worker.
    pub(super) fn writer(&self, protection: &Protection, context: &JobContext) -> io::Result<File> {
        check(context)?;
        protection
            .exclusive_rendezvous(&self.scope.root.path.join("writer"), context)
            .map_err(record::public_error)
    }
    /// Open one registered namespace while retaining its canonical ancestor chain.
    pub(super) fn namespace(&self, namespace: Namespace) -> io::Result<Directory> {
        Directory::open(&self.scope.root.path.join(namespace.directory()), false)
    }
}
