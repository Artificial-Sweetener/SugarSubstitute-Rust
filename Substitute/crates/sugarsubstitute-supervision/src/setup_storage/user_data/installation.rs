// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain protected installation correlation without acquiring run, trial or activation authority.
use super::*;
use crate::{
    durable_storage::{files::Directory, invalid, protected::directories::Mode, protection},
    setup_storage::{self, SelectionStore, backend_data::BackendDataAuthority},
};
use sugarsubstitute_contracts::{
    installation::{Candidate, Generation, InstallationRecord},
    lifecycle::NamespaceId,
};

pub(super) struct InstallationData {
    pub(super) root: Directory,
    pub(super) generation: Directory,
    pub(super) identity: Generation,
    pub(super) snapshot: InstallationRecord,
    pub(super) backend: BackendDataAuthority,
    _store: SelectionStore,
}
impl InstallationData {
    /// Share the running reader's exact missing-marker boundary; present damaged state never falls back.
    pub(super) fn open(image: &Path, context: &JobContext) -> io::Result<Option<Self>> {
        check(context)?;
        let layout =
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(image);
        if layout.root() == image {
            return Ok(None);
        }
        let root = Directory::open(layout.root(), false)?;
        if !setup_storage::ownership::running_present(&root, false, context)? {
            return Ok(None);
        }
        let Some(store) =
            SelectionStore::existing(root.path(), context).map_err(io::Error::other)?
        else {
            return Ok(None);
        };
        let snapshot = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is missing"))?;
        let name = image.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
            crate::durable_storage::invalid("Running generation name is not Unicode")
        })?;
        let identity = generations(&snapshot)
            .find(|generation| generation.id.as_str().eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| invalid("Running image has no complete selected generation"))?;
        let generation = protection::with(context, |owner| {
            let path = root.path().join("releases").join(identity.id.as_str());
            owner.directory(&path, Mode::Existing, context)?;
            let generation = Directory::open(&path, false)?;
            if generation.native_identity()? != Directory::open(image, false)?.native_identity()? {
                return Err(invalid("Running image differs from protected selected generation"));
            }
            Ok(generation)
        })?;
        let backend = BackendDataAuthority::read(&root, &generation, &identity, context)?;
        check(context)?;
        Ok(Some(Self { root, generation, identity, snapshot, backend, _store: store }))
    }

    /// Keep each complete selected generation and Ready record pinned through namespace admission.
    /// The unchanged hash consumes only the actual account SID and full native generation identity.
    pub(super) fn with_namespaces<T>(
        &self,
        context: &JobContext,
        operation: impl FnOnce(&[NamespaceId]) -> io::Result<T>,
    ) -> io::Result<T> {
        self.require_unchanged_generations(context)?;
        let mut pins = Vec::new();
        let mut namespaces = Vec::new();
        let result = protection::with(context, |owner| {
            for generation in generations(&self.snapshot) {
                let path = self.root.path().join("releases").join(generation.id.as_str());
                owner.directory(&path, Mode::Existing, context)?;
                let directory = Directory::open(&path, false)?;
                let (_, record) = setup_storage::runtime_record::read(
                    &self.root,
                    &directory,
                    Some(&generation.package),
                    context,
                )?;
                namespaces.push(owner.generation_namespace(&directory, context)?);
                pins.push((directory, record));
            }
            operation(&namespaces)
        })?;
        self.require_unchanged_generations(context)?;
        Ok(result)
    }

    /// Settlement correlates the generations read at open; the record is not held open, so a
    /// publication that changed that generation set since then fails this settlement closed.
    fn require_unchanged_generations(&self, context: &JobContext) -> io::Result<()> {
        let current = self
            ._store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is missing"))?;
        if generations(&current).ne(generations(&self.snapshot)) {
            return Err(invalid("Installation generations changed during data settlement"));
        }
        Ok(())
    }
}

/// Staging never supplies a complete generation or a historical namespace association.
fn generations(record: &InstallationRecord) -> impl Iterator<Item = &Generation> {
    record.accepted().into_iter().chain(record.retained()).chain(record.candidate().and_then(
        |candidate| match candidate {
            Candidate::Prepared { generation } => Some(generation),
            Candidate::Trial { attempt } => Some(&attempt.generation),
            Candidate::Staging { .. } => None,
        },
    ))
}
