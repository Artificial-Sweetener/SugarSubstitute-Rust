// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound sidecars and owner markers while sharing actual protected file publication mechanics.
use super::*;
use crate::durable_storage::streamed_record::StreamStage;
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Owned bounded source bytes keep their allocation funded after an individual job returns.
pub(in crate::setup_storage) struct RecordBytes {
    pub(in crate::setup_storage) bytes: Vec<u8>,
    pub(in crate::setup_storage) memory: MemoryClaim,
}
impl GeneratedFiles {
    /// Initialize a marked directory only when absent/empty or already carrying this exact marker.
    pub(in crate::setup_storage) fn initialize(
        &mut self,
        directory: &str,
        marker: &str,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<PathBuf> {
        files::flat_leaf(marker)?;
        let name = format!("{directory}/{marker}");
        self.check(context)?;
        self.index.admit(directory, true, 0)?;
        if self.index.charge(&name)?.is_none() {
            match self.parent(&name, false, context) {
                Ok(parent) => {
                    if parent.entries(context)?.next().transpose()?.is_some() {
                        return Err(invalid("Generated directory contains unowned files"));
                    }
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
        }
        self.immutable(&name, bytes, context)?;
        Ok(self.lease.path().join(directory))
    }
    /// Observe a closed admitted file through its actual regular single-link native handle.
    pub(in crate::setup_storage) fn length(
        &mut self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<Option<u64>> {
        self.check(context)?;
        self.inactive(name)?;
        let Some(charge) = self.index.charge(name)? else { return Ok(None) };
        let parent = self.parent(name, false, context)?;
        let file = match parent
            .read(name.rsplit('/').next().ok_or_else(|| invalid("File leaf absent"))?)
        {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause),
        };
        files::single_link(&file)?;
        protection::with(context, |protection| protection.verify_file(&file))?;
        let length = files::length(&file, context)?;
        self.index.admit(name, false, charge.max(length))?;
        check(context)?;
        Ok(Some(length))
    }
    /// Read an admitted, regular single-link record with room for bounded overflow detection.
    pub(in crate::setup_storage) fn read(
        &self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<RecordBytes> {
        self.check(context)?;
        self.inactive(name)?;
        if self.index.charge(name)?.is_none() {
            return Err(io::ErrorKind::NotFound.into());
        }
        let memory = context.memory().claim(16 * 1024).map_err(io::Error::other)?;
        let parent = self.parent(name, false, context)?;
        let mut file =
            parent.read(name.rsplit('/').next().ok_or_else(|| invalid("Record leaf absent"))?)?;
        files::single_link(&file)?;
        protection::with(context, |protection| protection.verify_file(&file))?;
        let bytes = files::read_bounded(&mut file, 8192)?;
        check(context)?;
        Ok(RecordBytes { bytes, memory })
    }
    /// Admit the original target plus the complete stage before any replacement effects.
    pub(in crate::setup_storage) fn write(
        &mut self,
        name: &str,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.check(context)?;
        self.inactive(name)?;
        if bytes.len() > 8192 {
            return Err(invalid("Download record exceeds its bound"));
        }
        let old = self.index.charge(name)?.unwrap_or(0);
        self.index.admit(name, false, old)?;
        let staged_name = StreamStage::unique_name()?;
        let relative = match name.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/{staged_name}"),
            None => staged_name.clone(),
        };
        if self.index.charge(&relative)?.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        self.index.admit(&relative, false, bytes.len() as u64)?;
        let parent = self.parent(name, true, context)?;
        let leaf = name.rsplit('/').next().ok_or_else(|| invalid("Record leaf absent"))?;
        let stage = StreamStage::named(&parent, leaf, &staged_name, bytes.len() as u64, context)?;
        let mut output = stage.create(context)?;
        output.write_with(context, |output| output.write_all(bytes))?;
        output.commit(context)?;
        self.index.removed(&relative)?;
        self.index.admit(name, false, bytes.len() as u64)
    }
    /// Publish an immutable owner marker, comparing exact existing bytes rather than replacing it.
    pub(in crate::setup_storage) fn immutable(
        &mut self,
        name: &str,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.check(context)?;
        self.inactive(name)?;
        if bytes.len() > 8192 {
            return Err(invalid("Download marker exceeds its bound"));
        }
        let old = self.index.charge(name)?.unwrap_or(0);
        self.index.admit(name, false, old.max(bytes.len() as u64))?;
        let parent = self.parent(name, true, context)?;
        protection::with(context, |protection| {
            let path = parent
                .path()
                .join(name.rsplit('/').next().ok_or_else(|| invalid("Marker leaf absent"))?);
            match protection.file(&path, context) {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(bytes)?;
                    files::sync(&file, context)?;
                    check(context)
                }
                Err(cause) if record::collision(&cause) => {
                    let mut file = parent.read(
                        name.rsplit('/').next().ok_or_else(|| invalid("Marker leaf absent"))?,
                    )?;
                    files::single_link(&file)?;
                    protection.verify_file(&file)?;
                    if files::read_bounded(&mut file, 8192)? != bytes {
                        return Err(invalid("Download marker identity changed"));
                    }
                    check(context)
                }
                Err(cause) => Err(cause),
            }
        })?;
        self.index.admit(name, false, bytes.len() as u64)
    }
    /// Retire only a closed regular file through identity-checked native deletion.
    pub(in crate::setup_storage) fn remove(
        &mut self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<()> {
        self.check(context)?;
        self.inactive(name)?;
        if self.index.charge(name)?.is_none() {
            return Ok(());
        }
        let parent = self.parent(name, false, context)?;
        let leaf = name.rsplit('/').next().ok_or_else(|| invalid("Record leaf absent"))?;
        let file = match parent.read(leaf) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                return self.index.removed(name);
            }
            Err(cause) => return Err(cause),
        };
        protection::with(context, |protection| protection.verify_file(&file))?;
        let identity = files::file_identity(&file)?;
        drop(file);
        retirement::file(&parent, leaf, identity)?;
        self.index.removed(name)
    }
    /// Shared-exclusive payload handles must close before any other operation on that filename.
    fn inactive(&self, name: &str) -> io::Result<()> {
        if self.payloads.contains_key(name) {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Download writer active"));
        }
        Ok(())
    }
}
