// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind backend storage policy to retained installer and managed-provisioning provenance.
mod provisioning;
#[cfg(test)]
mod tests;
use super::{check, invalid, runtime_record};
use crate::durable_storage::{files::Directory, protection};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    installation::Generation,
};

/// Storage ownership is independent of who starts the backend process.
#[derive(Clone, Copy)]
pub enum BackendStorage<'a> {
    /// Preserve captured storage settings of an explicitly selected external ComfyUI.
    External,
    /// Use storage whose installed/provisioned ownership the native owner has verified.
    InstallationOwned(&'a OwnedBackendStorage),
}

/// Opaque installation-owned data selection; paths and launch flags cannot construct it.
pub struct OwnedBackendStorage {
    source: Directory,
    directory: PathBuf,
    _storage: Directory,
    _inputs: Vec<File>,
}
impl OwnedBackendStorage {
    /// Borrow the confined spelling while this authority retains its native ancestry.
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

pub(super) struct BackendDataAuthority {
    owned: Option<OwnedBackendStorage>,
    _record: File,
    _marker: File,
    _memory: MemoryClaim,
}
impl BackendDataAuthority {
    /// Reuse the original schema-one codec under actual verified generation/run custody.
    /// Missing/contradictory installed provenance never becomes an external-data fallback.
    pub(super) fn read(
        root: &Directory,
        generation: &Directory,
        expected: &Generation,
        context: &JobContext,
    ) -> io::Result<Self> {
        let memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
        let marker = super::ownership::read(root)?;
        let (record, file) = runtime_record::read(root, generation, Some(expected), context)?;
        let owned = match record.selection.mode.as_str() {
            "attached" => None,
            "managed" => {
                let source = Directory::open(&record.runtime.directory, false)?;
                let inputs = provisioning::verify(&record.selection.comfy_root, &source, context)?;
                let layout =
                    sugarsubstitute_configuration::layout::InstallationLayout::new(root.path());
                let directory = layout.comfy_data().join("runtime");
                let storage = protection::with(context, |owner| {
                    let mut path = layout.user();
                    for component in [None, Some("comfy-data"), Some("runtime")] {
                        if let Some(component) = component {
                            path.push(component);
                        }
                        owner.directory(
                            &path,
                            crate::durable_storage::protected::directories::Mode::OpenOrCreate,
                            context,
                        )?;
                    }
                    Directory::open(&directory, false)
                })?;
                Some(OwnedBackendStorage { source, directory, _storage: storage, _inputs: inputs })
            }
            _ => return Err(invalid("Unsupported installer backend data provenance")),
        };
        check(context)?;
        Ok(Self { owned, _record: file, _marker: marker, _memory: memory })
    }

    /// Compare actual no-reparse native directory identities on the admitted projection worker.
    pub(super) fn select(
        &self,
        requested: &Path,
        context: &JobContext,
    ) -> io::Result<BackendStorage<'_>> {
        check(context)?;
        let Some(owned) = &self.owned else {
            return Ok(BackendStorage::External);
        };
        let selected = Directory::open(requested, false)?;
        let same = selected.identity()? == owned.source.identity()?;
        check(context)?;
        Ok(if same { BackendStorage::InstallationOwned(owned) } else { BackendStorage::External })
    }
}
