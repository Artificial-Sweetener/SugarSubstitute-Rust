// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read and write a repository folder, signing keys and release targets for the command line.
//!
//! A repository folder holds the published files under their flat names. New versioned files
//! are created exclusively, so a published version is never rewritten; `timestamp.json` is
//! replaced through a rename, after every other file is written.
use crate::{
    Error, Files, Published, invalid, keys::SigningKey, sign::NewTarget, verify::Payloads,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::update::MAX_OFFER_BYTES;
use sugarsubstitute_update::{MAX_METADATA_BYTES, MAX_TARGET_BYTES};

const CHUNK_BYTES: usize = 1024 * 1024;

/// Read every metadata-sized `.json` file of a folder by name.
/// # Errors
/// Rejects unreadable folders and JSON files larger than installations accept.
pub fn read_files(folder: &Path) -> Result<Files, Error> {
    let mut files = Files::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let name =
            entry.file_name().into_string().map_err(|_| invalid("A file name is not UTF-8"))?;
        if !name.ends_with(".json") || !entry.file_type()?.is_file() {
            continue;
        }
        if entry.metadata()?.len() > MAX_METADATA_BYTES as u64 {
            return Err(invalid(format!("{name} is larger than installations accept")));
        }
        files.insert(name, fs::read(entry.path())?);
    }
    Ok(files)
}

/// Read one bounded metadata file, such as a trusted root.
/// # Errors
/// Rejects unreadable and oversized files.
pub fn read_metadata(path: &Path) -> Result<Vec<u8>, Error> {
    if fs::metadata(path)?.len() > MAX_METADATA_BYTES as u64 {
        return Err(invalid(format!("{} is larger than installations accept", path.display())));
    }
    Ok(fs::read(path)?)
}

/// Write published files into `folder`: versioned files exclusively, then replaced channel
/// descriptors, then `timestamp.json`. Returns the names written, in writing order, which is
/// also the order to upload them in.
/// # Errors
/// Refuses to rewrite an existing versioned file; returns write failures.
pub fn write(folder: &Path, published: &Published) -> Result<Vec<String>, Error> {
    let versioned: Vec<(&String, &Vec<u8>)> =
        published.files.iter().filter(|(name, _)| *name != "timestamp.json").collect();
    // Refuse before writing anything, so a refusal leaves the folder exactly as it was.
    for (name, _) in &versioned {
        if fs::symlink_metadata(folder.join(name)).is_ok() {
            return Err(invalid(format!("{name} is already published and is never rewritten")));
        }
    }
    let mut written = Vec::new();
    for (name, bytes) in versioned {
        let mut file =
            fs::OpenOptions::new().write(true).create_new(true).open(folder.join(name)).map_err(
                |cause| match cause.kind() {
                    io::ErrorKind::AlreadyExists => {
                        invalid(format!("{name} is already published and is never rewritten"))
                    }
                    _ => cause.into(),
                },
            )?;
        file.write_all(bytes)?;
        file.sync_all()?;
        written.push(name.clone());
    }
    for (name, bytes) in &published.descriptors {
        replace(folder, name, bytes)?;
        written.push(name.clone());
    }
    if let Some(bytes) = published.files.get("timestamp.json") {
        replace(folder, "timestamp.json", bytes)?;
        written.push("timestamp.json".into());
    }
    Ok(written)
}

/// Replace one file through a rename, so a reader sees the old or the new bytes, never a mix.
fn replace(folder: &Path, name: &str, bytes: &[u8]) -> Result<(), Error> {
    let staged = folder.join(format!(".{name}.signing"));
    let mut file = fs::File::create(&staged)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&staged, folder.join(name))?;
    Ok(())
}

/// Write the names of written files, one per line, for the workflow to upload in that order.
/// # Errors
/// Returns write failures.
pub fn write_manifest(path: &Path, names: &[String]) -> Result<(), Error> {
    let text: String = names.iter().map(|name| format!("{name}\n")).collect();
    fs::write(path, text)?;
    Ok(())
}

/// Resolve a key reference: `env:NAME` reads an environment variable, `file:PATH` a file.
/// # Errors
/// Rejects other forms, missing or empty values and malformed keys.
pub fn read_key(reference: &str) -> Result<SigningKey, Error> {
    let text = if let Some(name) = reference.strip_prefix("env:") {
        std::env::var(name).map_err(|_| invalid(format!("The key variable {name} is not set")))?
    } else if let Some(path) = reference.strip_prefix("file:") {
        fs::read_to_string(path)?
    } else {
        return Err(invalid("A key is given as env:NAME or file:PATH"));
    };
    if text.trim().is_empty() {
        return Err(invalid(format!("The key {reference} is empty")));
    }
    SigningKey::from_hex(&text)
}

/// Write a new key's seed to a file that must not exist yet, readable only by its owner where
/// the platform supports that.
/// # Errors
/// Refuses an existing file; returns write failures.
pub fn write_key(path: &Path, seed: &str) -> Result<(), Error> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(format!("{seed}\n").as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Describe a file to publish under its own name by streaming its length and SHA-256.
/// # Errors
/// Rejects unreadable, non-UTF-8-named and oversized files.
pub fn describe_target(path: &Path) -> Result<NewTarget, Error> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(format!("{} has no UTF-8 file name", path.display())))?
        .to_owned();
    let mut digest = Sha256::new();
    let mut length: u64 = 0;
    stream_file(path, &mut |chunk| {
        digest.update(chunk);
        length = length.saturating_add(chunk.len() as u64);
        if length > MAX_TARGET_BYTES {
            return Err(invalid(format!("{name} is larger than installations accept")));
        }
        Ok(())
    })?;
    let descriptor = if name.ends_with(".json") {
        if length > MAX_OFFER_BYTES as u64 {
            return Err(invalid(format!("{name} is larger than a channel descriptor may be")));
        }
        Some(fs::read(path)?)
    } else {
        None
    };
    let sha256: [u8; 32] = digest.finalize().into();
    if descriptor.as_ref().is_some_and(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)) != sha256) {
        return Err(invalid(format!("{name} changed while it was read")));
    }
    Ok(NewTarget { name, length, sha256, descriptor })
}

/// Packages supplied as files, by their published name.
pub struct PayloadFiles {
    paths: std::collections::BTreeMap<String, PathBuf>,
}
impl PayloadFiles {
    /// No packages.
    pub fn none() -> Self {
        Self { paths: std::collections::BTreeMap::new() }
    }

    /// Every regular file in `folder`.
    /// # Errors
    /// Returns read failures and non-UTF-8 names.
    pub fn folder(folder: &Path) -> Result<Self, Error> {
        let mut paths = std::collections::BTreeMap::new();
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("A file name is not UTF-8"))?;
                paths.insert(name, entry.path());
            }
        }
        Ok(Self { paths })
    }

    /// Exactly these files, under their own names.
    /// # Errors
    /// Rejects paths without a UTF-8 file name.
    pub fn files(files: &[PathBuf]) -> Result<Self, Error> {
        let mut paths = std::collections::BTreeMap::new();
        for path in files {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| invalid(format!("{} has no UTF-8 file name", path.display())))?;
            paths.insert(name.to_owned(), path.clone());
        }
        Ok(Self { paths })
    }
}
impl Payloads for PayloadFiles {
    fn stream(
        &self,
        name: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let Some(path) = self.paths.get(name) else { return Ok(false) };
        stream_file(path, sink)?;
        Ok(true)
    }
}

fn stream_file(path: &Path, sink: &mut dyn FnMut(&[u8]) -> Result<(), Error>) -> Result<(), Error> {
    let mut file = fs::File::open(path)?;
    let mut buffer = vec![0; CHUNK_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        sink(buffer.get(..read).unwrap_or_default())?;
    }
}
