// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact distribution files authenticated by the installer or a verified update archive.
use super::super::{
    check,
    files::{self, Directory},
    invalid,
};
use super::manifest::AuthenticatedManifest;
use std::os::windows::fs::FileExt;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

use sugarsubstitute_update::package::Manifest;
/// Retain a verified file together with all native directory handles needed to pin its path.
pub(super) struct Input {
    pub(super) file: File,
    pub(super) _parent: Directory,
}

/// A complete authenticated package with retained read-only source files and ancestry.
/// Dropping it releases custody only; it never deletes package or user data.
pub(in crate::setup_storage) struct Payload {
    pub(in crate::setup_storage) compatibility: super::super::compatibility::Compatibility,
    pub(in crate::setup_storage) root: PathBuf,
    digest: String,
    pub(in crate::setup_storage) inventory: Manifest,
    pub(super) manifest_bytes: Vec<u8>,
    inputs: Vec<Input>,
    _manifest: File,
    _root: Directory,
    _memory: MemoryClaim,
}
impl Payload {
    /// Extract a signed update using the installer's package contract and retain all verified files.
    /// The source is the original exclusively held download, never a caller-supplied path/hash.
    /// The caller retains that archive independently of these verified payload handles.
    /// Reserve `RESERVATION_BYTES` in addition to the retained download's storage claim.
    /// # Errors
    /// Rejects unsafe archives, incomplete distributions, altered files and cancellation.
    /// Failed attempts retain their files; retry creates a separate extraction generation.
    pub(in crate::setup_storage) fn from_download(
        source: &mut super::super::VerifiedDownload,
        workspace: &Path,
        component: sugarsubstitute_contracts::update::Component,
        context: &JobContext,
    ) -> io::Result<Self> {
        let (root, digest) =
            super::super::packed::downloaded(source, workspace, component, context)?;
        let bundle = tracing::trace_span!(target: "sugarsubstitute::storage", "bundle_verify")
            .in_scope(|| Self::open(&root, &digest, component, context))?;
        check(context)?;
        Ok(bundle)
    }
    /// Verify every package byte before starting its provisioning worker.
    /// `expected` must be compiled into the trusted installer, never read from its directory.
    /// Run on an admitted blocking worker; cancellation is checked between streamed blocks.
    /// # Errors
    /// Rejects unsigned builds, malformed manifests, aliases, changed bytes and reparse points.
    pub(in crate::setup_storage) fn open(
        root: &Path,
        expected: &str,
        component: sugarsubstitute_contracts::update::Component,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let memory =
            context.memory().claim(super::super::RESERVATION_BYTES).map_err(io::Error::other)?;
        let authenticated = AuthenticatedManifest::open(root, expected, component, context)?;
        let manifest = authenticated.inventory;
        let names = manifest.names();
        let mut inputs = Vec::new();
        let mut compatibility = super::super::compatibility::Compatibility::default();
        for entry in manifest.entries() {
            check(context)?;
            let path = root.join(entry.path());
            let parent = Directory::open(
                path.parent().ok_or_else(|| invalid("Payload parent absent"))?,
                false,
            )?;
            let leaf = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid("Payload leaf absent"))?;
            let mut file = parent.read(leaf)?;
            if file.metadata()?.len() != entry.bytes()
                || hash(&mut file, context)? != entry.digest()
            {
                return Err(invalid("Installer payload verification failed"));
            }
            compatibility.inspect_verified(entry.path(), &mut file)?;
            inputs.push(Input { file, _parent: parent });
        }
        super::super::inventory::verify(root, &names, || check(context))?;
        match component {
            sugarsubstitute_contracts::update::Component::Application => {
                compatibility.validate_pair()?
            }
            sugarsubstitute_contracts::update::Component::Updater => {
                compatibility.require_updater()?
            }
        }
        Ok(Self {
            compatibility,
            root: root.into(),
            digest: authenticated.digest,
            inventory: manifest,
            manifest_bytes: authenticated.bytes,
            inputs,
            _manifest: authenticated.file,
            _root: authenticated.directory,
            _memory: memory,
        })
    }
    /// Immutable generation identity used by installation transactions.
    pub(in crate::setup_storage) fn digest(&self) -> &str {
        &self.digest
    }
    /// Release label authenticated by the installer package.
    pub(in crate::setup_storage) fn release(&self) -> &str {
        self.inventory.release()
    }

    /// Copy retained source bytes using create-only publication; existing exact files resume.
    /// The caller supplies the destination's protection policy on the admitted storage worker.
    pub(in crate::setup_storage) fn copy(
        &self,
        prefix: &str,
        target: &Path,
        context: &JobContext,
        create: impl Fn(&Path) -> io::Result<File>,
    ) -> io::Result<()> {
        let _operation =
            tracing::trace_span!(target: "sugarsubstitute::storage", "bundle_copy").entered();
        let _directory = Directory::open(target, true)?;
        for (entry, input) in self.inventory.entries().iter().zip(&self.inputs) {
            let Some(relative) = entry.path().strip_prefix(prefix) else {
                continue;
            };
            check(context)?;
            let output = target.join(relative);
            let parent = Directory::open(
                output.parent().ok_or_else(|| invalid("Output parent absent"))?,
                true,
            )?;
            let leaf = output
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid("Payload leaf absent"))?;
            match parent.read(leaf) {
                Ok(mut original) => {
                    if original.metadata()?.len() != entry.bytes()
                        || hash(&mut original, context)? != entry.digest()
                    {
                        return Err(invalid("Existing copied payload differs from its package"));
                    }
                    continue;
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
            let staged = super::super::staging::unique_path(&parent, ".payload-stage")?;
            let mut destination = create(&staged)?;
            let mut offset = 0;
            let mut buffer = [0_u8; 65536];
            loop {
                check(context)?;
                let count = input.file.seek_read(&mut buffer, offset)?;
                if count == 0 {
                    break;
                }
                destination.write_all(&buffer[..count])?;
                offset += count as u64;
            }
            if offset != entry.bytes() {
                return Err(invalid("Payload length changed while copying"));
            }
            destination.sync_all()?;
            drop(destination);
            check(context)?;
            super::super::staging::publish(&staged, &output)?;
        }
        Ok(())
    }

    /// Retain the exact authenticated manifest beside a copied complete package tree.
    pub(in crate::setup_storage) fn retain_manifest(
        &self,
        target: &Path,
        create: impl Fn(&Path) -> io::Result<File>,
    ) -> io::Result<()> {
        files::write_immutable(&target.join("setup-manifest.json"), &self.manifest_bytes, create)
    }

    /// Require account protection on every retained installed file, including its inventory.
    /// Extraction inputs need authenticity and custody; installed engine generations also need this policy.
    pub(in crate::setup_storage) fn require_protected(
        &self,
        protection: &super::super::protection::Protection,
    ) -> io::Result<()> {
        protection.verify_file(&self._manifest)?;
        for input in &self.inputs {
            protection.verify_file(&input.file)?;
        }
        Ok(())
    }
}

/// Stream a cancellable digest and rewind the same retained handle for subsequent inspection.
pub(super) fn hash(file: &mut File, context: &JobContext) -> io::Result<String> {
    let mut digest = sugarsubstitute_update::package::PayloadHasher::new();
    let mut buffer = [0_u8; 65536];
    loop {
        check(context)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(digest.finish_hex())
}
