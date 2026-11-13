// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact distribution files authenticated by the installer or a verified update archive.
use super::super::check;
use super::super::invalid;
use super::manifest::AuthenticatedManifest;
use crate::durable_storage::files::{self, Directory};
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
    cache: Option<std::sync::Arc<super::super::cache::CacheLease>>,
}
impl Payload {
    /// Retain immutable cache custody through the actual native process family.
    pub(super) fn bind_native_cache(
        &self,
        family: super::super::cache_custody::CacheFamilyId,
        context: &JobContext,
    ) -> io::Result<Option<super::super::cache::NativeCachePin>> {
        self.cache.as_ref().map(|cache| cache.bind_native(family, context)).transpose()
    }

    /// Extract a signed update using the installer's package contract and retain all verified files.
    /// The source is the original exclusively held download, never a caller-supplied path/hash.
    /// The caller retains that archive independently of these verified payload handles.
    /// Reserve `RESERVATION_BYTES` in addition to the retained download's storage claim.
    /// # Errors
    /// Rejects unsafe archives, incomplete distributions, altered files and cancellation.
    /// Failed attempts retain their files; retry creates a separate extraction generation.
    pub(in crate::setup_storage) fn from_download(
        source: &mut super::super::VerifiedDownload,
        cache: &super::super::cache::RuntimeCache,
        component: sugarsubstitute_contracts::update::Component,
        context: &JobContext,
    ) -> io::Result<Self> {
        let bundle = super::super::packed::downloaded(source, cache, component, context)?;
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
        Self::open_pinned(root, expected, component, None, context)
    }
    /// Authenticate cache content while retaining the exact immutable storage lease.
    pub(in crate::setup_storage) fn open_pinned(
        root: &Path,
        expected: &str,
        component: sugarsubstitute_contracts::update::Component,
        cache: Option<std::sync::Arc<super::super::cache::CacheLease>>,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let memory =
            context.memory().claim(super::super::RESERVATION_BYTES).map_err(io::Error::other)?;
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid(
                "This installer has not been packaged with authenticated payloads",
            ));
        }
        let directory = Directory::open(root, false)?;
        let directory = if let Some(lease) = &cache {
            if directory.identity()? != Directory::open(&lease.path(), false)?.identity()? {
                return Err(invalid("Payload and cache lease identities differ"));
            }
            directory
        } else {
            unmanaged_payload(directory)?
        };
        let root = directory.path().to_path_buf();
        let authenticated =
            AuthenticatedManifest::open_directory(directory, expected, component, context)?;
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
            if files::length(&file, context)? != entry.bytes()
                || hash(&mut file, context)? != entry.digest()
            {
                return Err(invalid("Installer payload verification failed"));
            }
            compatibility.inspect_verified(entry.path(), &mut file)?;
            inputs.push(Input { file, _parent: parent });
        }
        super::super::inventory::verify(&root, &names, context)?;
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
            root,
            digest: authenticated.digest,
            inventory: manifest,
            manifest_bytes: authenticated.bytes,
            inputs,
            _manifest: authenticated.file,
            _root: authenticated.directory,
            _memory: memory,
            cache,
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
    /// Before writing, the volume must hold every byte still to copy plus
    /// [`crate::setup_storage::HEADROOM_BYTES`]. A stage file that is not published, because the
    /// copy failed or was cancelled, is removed again; only a power loss can leave one, and
    /// then it is preserved as unknown content.
    /// # Errors
    /// A `StorageFull` error caused by [`crate::setup_storage::SpaceShortage`] when the disk is
    /// short, and every native, verification and cancellation failure.
    pub(in crate::setup_storage) fn copy(
        &self,
        prefix: &str,
        target: &Path,
        context: &JobContext,
        create: impl Fn(&Path) -> io::Result<File>,
    ) -> io::Result<()> {
        self.copy_with(prefix, target, context, create, &crate::setup_storage::VolumeSpace)
    }

    /// [`Self::copy`] measuring free space through `space`.
    pub(in crate::setup_storage) fn copy_with(
        &self,
        prefix: &str,
        target: &Path,
        context: &JobContext,
        create: impl Fn(&Path) -> io::Result<File>,
        space: &dyn crate::setup_storage::FreeSpace,
    ) -> io::Result<()> {
        let _operation =
            tracing::trace_span!(target: "sugarsubstitute::storage", "bundle_copy").entered();
        let directory = Directory::open(target, true)?;
        let remaining = self.uncopied_bytes(prefix, target)?;
        crate::setup_storage::download::space::ensure(
            directory.path(),
            remaining,
            space,
            &mut [],
            context,
        )?;
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
                    if files::length(&original, context)? != entry.bytes()
                        || hash(&mut original, context)? != entry.digest()
                    {
                        return Err(invalid("Existing copied payload differs from its package"));
                    }
                    continue;
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
            let staged = crate::durable_storage::staging::unique_path(&parent, ".payload-stage")?;
            let mut stage = Stage::created(&parent, &staged, create(&staged)?)?;
            let destination = stage.file()?;
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
            files::sync(destination, context)?;
            stage.close();
            check(context)?;
            crate::durable_storage::staging::publish(&staged, &output)?;
            stage.published();
        }
        Ok(())
    }

    /// Bytes of the entries below `prefix` whose output does not exist yet. An existing output
    /// is verified by the copy itself, so it counts as copied here.
    fn uncopied_bytes(&self, prefix: &str, target: &Path) -> io::Result<u64> {
        let mut bytes = 0_u64;
        for entry in self.inventory.entries() {
            let Some(relative) = entry.path().strip_prefix(prefix) else { continue };
            let output = target.join(relative);
            let present = match output.parent().map(|parent| Directory::open(parent, false)) {
                Some(Ok(parent)) => {
                    let leaf = output.file_name().and_then(|value| value.to_str());
                    leaf.is_some_and(|leaf| parent.read(leaf).is_ok())
                }
                Some(Err(_)) | None => false,
            };
            if !present {
                bytes = bytes.saturating_add(entry.bytes());
            }
        }
        Ok(bytes)
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
        protection: &crate::durable_storage::protection::Protection,
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

/// Require the cache owner's original lease even when a payload arrived through a drive alias.
fn unmanaged_payload(directory: Directory) -> io::Result<Directory> {
    let resolved = directory.resolved()?;
    for part in resolved.path().components() {
        if let std::path::Component::Normal(name) = part {
            let name = name.to_string_lossy().to_ascii_lowercase();
            if name.strip_prefix("runtime-cache-v").is_some_and(|version| {
                !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
            }) {
                return Err(invalid("Cached payload requires its original storage lease"));
            }
        }
    }
    Ok(resolved)
}

/// A stage file this copy created, with its writable handle. Unless it was published under its
/// final name, dropping the guard closes the handle and then removes exactly that file again,
/// by its native identity.
struct Stage<'a> {
    parent: &'a Directory,
    file: Option<File>,
    leaf: Option<String>,
    identity: (u32, u32, u32),
}
impl<'a> Stage<'a> {
    fn created(parent: &'a Directory, staged: &Path, file: File) -> io::Result<Self> {
        let leaf = staged
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| invalid("Stage leaf absent"))?
            .to_owned();
        let identity = files::file_identity(&file);
        let mut stage = Self { parent, file: Some(file), leaf: Some(leaf), identity: (0, 0, 0) };
        stage.identity = identity?;
        Ok(stage)
    }

    /// The writable stage handle until [`Self::close`].
    fn file(&mut self) -> io::Result<&mut File> {
        self.file.as_mut().ok_or_else(|| invalid("Stage file already closed"))
    }

    /// Close the stage handle before the stage is renamed into place.
    fn close(&mut self) {
        self.file = None;
    }

    /// The stage file is now the published output and must stay.
    fn published(&mut self) {
        self.leaf = None;
    }
}
impl Drop for Stage<'_> {
    fn drop(&mut self) {
        self.file = None;
        let Some(leaf) = self.leaf.take() else { return };
        if let Err(cause) =
            crate::durable_storage::retirement::file(self.parent, &leaf, self.identity)
        {
            tracing::warn!(operation = "bundle_stage_cleanup", cause = %cause,
                "An unpublished stage file could not be removed; it is preserved");
        }
    }
}
