// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independently pin protected final copies without rereading mutable original inputs.
use super::{
    manifest::{self, Manifest, Original},
    native::Producer,
    *,
};
use crate::durable_storage::{
    files,
    protected::{directories::Mode, record_file},
    protection,
};
use sha2::{Digest, Sha256};
use std::fs::File;

pub(super) struct PinnedOriginals {
    pub(super) manifest: Option<Manifest>,
    pub(super) startup: Vec<u8>,
    pub(super) descriptor: Option<Vec<u8>>,
    pub(super) directory: Directory,
    pins: Vec<File>,
}
impl PinnedOriginals {
    pub(super) fn new(parent: &Directory, reference: EntryOriginalsReference) -> io::Result<Self> {
        let name = record_file::hex(reference.manifest_sha256());
        let directory = parent.child(&name)?.retained()?.resolved()?;
        let mut pins = Vec::new();
        pins.try_reserve_exact(3).map_err(io::Error::other)?;
        Ok(Self { manifest: None, startup: Vec::new(), descriptor: None, directory, pins })
    }
    /// Adopt each passive file before any fallible inspection; the outer attempt retains failure.
    pub(super) fn load(
        &mut self,
        reference: EntryOriginalsReference,
        expected: &Producer,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        if !self.pins.is_empty() || self.manifest.is_some() {
            return Err(invalid("Invocation passive read already began"));
        }
        let directory = &self.directory;
        let pins = &mut self.pins;
        let result = protection::with(context, |owner| {
            owner.directory(directory.path(), Mode::Existing, context)?;
            pins.push(directory.read(manifest::MANIFEST)?);
            let file = pins.last_mut().ok_or_else(|| invalid("Invocation manifest pin absent"))?;
            files::single_link(file)?;
            owner.verify_file(file)?;
            let bytes = files::read_admitted(file, manifest::MAX_MANIFEST_BYTES, context)?;
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            if digest != *reference.manifest_sha256() {
                return Err(invalid("Invocation manifest digest differs from sealed reference"));
            }
            let manifest = Manifest::decode(&bytes)?;
            if manifest.producer != *expected
                || manifest.directory_identity != directory.native_identity()?
            {
                return Err(invalid(
                    "Invocation native producer or passive directory identity changed",
                ));
            }
            let startup = copy(directory, "startup.toml", &manifest.startup, pins, owner, context)?;
            let descriptor = manifest
                .descriptor
                .as_ref()
                .map(|value| copy(directory, "descriptor.json", value, pins, owner, context))
                .transpose()?;
            let mut visited = 0;
            for entry in directory.entries(context)? {
                let name = entry?.file_name();
                visited += 1;
                if visited > 3
                    || (name != manifest::MANIFEST
                        && name != "startup.toml"
                        && (name != "descriptor.json" || descriptor.is_none()))
                {
                    return Err(invalid("Invocation snapshot contains unknown original roles"));
                }
            }
            if visited != if descriptor.is_some() { 3 } else { 2 } {
                return Err(invalid("Invocation snapshot original roles are incomplete"));
            }
            manifest.intent(&startup, descriptor.as_deref())?;
            check(context)?;
            Ok((manifest, startup, descriptor))
        })?;
        self.manifest = Some(result.0);
        self.startup = result.1;
        self.descriptor = result.2;
        Ok(())
    }
}
fn copy(
    directory: &Directory,
    name: &str,
    expected: &Original,
    pins: &mut Vec<File>,
    owner: &protection::Protection,
    context: &JobContext,
) -> io::Result<Vec<u8>> {
    pins.push(directory.read(name)?);
    let file = pins.last_mut().ok_or_else(|| invalid("Invocation original pin absent"))?;
    files::single_link(file)?;
    owner.verify_file(file)?;
    let identity = files::native_identity(file)?;
    let bytes = files::read_admitted(
        file,
        sugarsubstitute_configuration::startup::MAX_STARTUP_CONFIGURATION_BYTES,
        context,
    )?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if identity != expected.copy_identity
        || bytes.len() as u64 != expected.bytes
        || digest != expected.sha256
    {
        return Err(invalid("Invocation passive original bytes or native identity changed"));
    }
    Ok(bytes)
}
