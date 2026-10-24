// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded disposable metadata records inventory ownership and restartable deletion progress.
use super::*;
mod names;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Persist exact disposable ownership without granting feature content authenticity.
pub(super) struct Record {
    pub(super) schema: u32,
    pub(super) identity: (u32, u32, u32),
    pub(super) namespace: Namespace,
    pub(super) key: String,
    pub(super) reserved: u64,
    pub(super) touched: u64,
    pub(super) deleting: bool,
    #[serde(default)]
    pub(super) staged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) producer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) consumers: Option<(u32, u32, u32)>,
    #[serde(deserialize_with = "bounded_files")]
    pub(super) files: Vec<String>,
    #[serde(deserialize_with = "bounded_files")]
    pub(super) directories: Vec<String>,
}
impl Record {
    /// Reject unsupported or ambiguous inventories before publication or deletion.
    pub(super) fn validate(&self, namespace: Namespace, key: &str) -> io::Result<()> {
        if self.schema != 1
            || self.namespace != namespace
            || self.key != key
            || !valid_entry_key(key)
            || self
                .producer
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 8192 || value.contains('\0'))
            || self.reserved == 0
            || self.reserved > namespace.budget()
            || self.files.len().saturating_add(self.directories.len()) > MAX_FILES
        {
            return Err(invalid("Cache entry schema, identity or reservation is invalid"));
        }
        names::validate(&self.files, &self.directories)
    }
    /// Index owned leaves without multiplying long shared ancestor paths in memory.
    pub(super) fn inventory_names(&self) -> io::Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        for name in self.files.iter().chain(&self.directories) {
            if name.split('/').count() > MAX_DEPTH {
                return Err(invalid("Cache inventory exceeds its directory-depth limit"));
            }
            names.insert(name.clone());
        }
        Ok(names)
    }
}
/// Constrain storage identities to canonical lowercase SHA-256 filenames.
pub(super) fn valid_key(key: &str) -> bool {
    key.len() == 64
        && key.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
/// A semantic key and fixed-width ordinal identify one independent disposable generation.
pub(super) fn valid_entry_key(key: &str) -> bool {
    key.split_once('-').is_some_and(|(semantic, generation)| {
        valid_key(semantic)
            && generation.len() == 16
            && generation.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
/// Read one protected bounded record and bind it to the retained native directory.
pub(super) fn load(
    directory: &Directory,
    namespace: Namespace,
    key: &str,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<Record> {
    let mut file =
        directory.open_existing("record", files::ExistingAccess::ReplaceableRead, context)?;
    protection.verify_file(&file)?;
    let record: Record =
        serde_json::from_slice(&files::read_admitted(&mut file, MAX_RECORD, context)?)?;
    record.validate(namespace, key)?;
    if directory.identity()? != record.identity {
        return Err(invalid("Cache entry native identity changed"));
    }
    Ok(record)
}
/// Flush a replacement before atomically publishing its complete inventory.
pub(super) fn save(directory: &Directory, record: &Record, context: &JobContext) -> io::Result<()> {
    record.validate(record.namespace, &record.key)?;
    let bytes = serde_json::to_vec(record)?;
    if bytes.len() > MAX_RECORD {
        return Err(invalid("Cache inventory exceeds metadata capacity"));
    }
    publish(directory, "record", &bytes, true, context)
}
/// Flush a fixed schema-owned journal slot before any producer output is removed.
pub(super) fn start_retirement(
    parent: &Directory,
    record: &Record,
    context: &JobContext,
) -> io::Result<()> {
    record.validate(record.namespace, &record.key)?;
    publish(
        parent,
        &format!("{}.retiring", record.key),
        &serde_json::to_vec(record)?,
        false,
        context,
    )
}
/// Reuse only a schema-owned scratch slot; interruption preserves the previous complete record.
pub(super) fn publish(
    directory: &Directory,
    name: &str,
    bytes: &[u8],
    replace: bool,
    context: &JobContext,
) -> io::Result<()> {
    crate::durable_storage::records::publish_reusable(
        directory,
        name,
        &format!("{name}.next"),
        bytes,
        MAX_RECORD,
        replace,
        context,
    )
}

/// Acquire an existing OS exclusion without creating or repairing an unknown rendezvous.
pub(super) fn exclusion(
    directory: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<File> {
    pin(directory, protection, true, context)
}
/// Retain a shared reader pin that conflicts with writers and cleanup across processes.
pub(super) fn reader(
    directory: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<File> {
    pin(directory, protection, false, context)
}
/// Cache policy requires an empty protected lease; storage owns actual access and sharing.
fn pin(
    directory: &Directory,
    protection: &Protection,
    writable: bool,
    context: &JobContext,
) -> io::Result<File> {
    let access = if writable {
        files::ExistingAccess::Exclusive
    } else {
        files::ExistingAccess::SharedMetadata
    };
    let file = directory.open_existing("lease", access, context)?;
    if files::length(&file, context)? != 0 {
        return Err(invalid("Cache lease must be empty"));
    }
    protection.verify_file(&file)?;
    check(context)?;
    Ok(file)
}
/// Initialize filesystem retention metadata from the same injected UTC clock as its record.
pub(super) fn initialize_time(file: &File, now: u64, context: &JobContext) -> io::Result<()> {
    let time = std::time::UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(now))
        .ok_or_else(|| invalid("Cache access time exceeded its supported range"))?;
    files::set_modified(file, time, context)
}
/// Read native retention metadata. It never grants package or installation authority.
pub(super) fn last_use(file: &File, context: &JobContext) -> io::Result<u64> {
    files::modified(file, context)?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(io::Error::other)
}
/// Clamp single-observation clock regression before updating cache-only native timestamps.
pub(super) fn touch(file: &File, now: u64, context: &JobContext) -> io::Result<()> {
    initialize_time(file, now.max(last_use(file, context)?), context)
}
/// Recognize original Windows sharing contention through retained error causes.
pub(super) fn busy(cause: &io::Error) -> bool {
    native_code(cause).is_some_and(|code| matches!(code, 32 | 33))
}
/// Preserve original native causes while exposing actionable storage and control categories.
/// Original control outcomes take priority over simultaneous native pressure or contention.
pub(in crate::setup_storage) fn public_error(cause: io::Error) -> io::Error {
    let control = kind(&cause);
    if matches!(control, io::ErrorKind::Interrupted | io::ErrorKind::TimedOut) {
        return if control == cause.kind() { cause } else { io::Error::new(control, cause) };
    }
    let category = if busy(&cause) {
        io::ErrorKind::WouldBlock
    } else {
        native_code(&cause)
            .map_or_else(|| kind(&cause), |code| io::Error::from_raw_os_error(code).kind())
    };
    if category == cause.kind() { cause } else { io::Error::new(category, cause) }
}
/// Recover the native code without replacing any original storage failure.
fn native_code(cause: &(dyn std::error::Error + 'static)) -> Option<i32> {
    if cause
        .downcast_ref::<crate::ClaimError>()
        .is_some_and(|cause| cause.cleanup_cause().is_some())
    {
        return None;
    }
    if let Some(io) = cause.downcast_ref::<io::Error>() {
        if let Some(code) = io.raw_os_error() {
            return Some(code);
        }
        if let Some(inner) = io.get_ref() {
            return native_code(inner);
        }
    }
    if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
        let code = native.code().0 as u32;
        if code & 0xffff_0000 == 0x8007_0000 {
            return Some((code & 0xffff) as i32);
        }
    }
    cause.source().and_then(native_code)
}
/// Preserve storage categories through protected-operation cause wrappers.
pub(super) fn kind(cause: &(dyn std::error::Error + 'static)) -> io::ErrorKind {
    if cause
        .downcast_ref::<crate::ClaimError>()
        .is_some_and(|cause| cause.cleanup_cause().is_some())
    {
        return io::ErrorKind::Other;
    }
    if let Some(io) = cause.downcast_ref::<io::Error>() {
        if io.kind() != io::ErrorKind::Other {
            return io.kind();
        }
        if let Some(inner) = io.get_ref() {
            return kind(inner);
        }
        return io.kind();
    }
    cause.source().map_or(io::ErrorKind::Other, kind)
}

/// Stop decoding inventory elements before they exceed admitted metadata capacity.
fn bounded_files<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Vec<String>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded cache file inventory")
        }
        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: S,
        ) -> Result<Self::Value, S::Error> {
            let mut files = Vec::new();
            while let Some(name) = sequence.next_element::<String>()? {
                if files.len() == MAX_FILES {
                    return Err(serde::de::Error::custom("Cache inventory exceeds its file limit"));
                }
                files.push(name.into_boxed_str().into_string());
            }
            Ok(files)
        }
    }
    decoder.deserialize_seq(Bounded)
}

/// Only a pure destination collision permits another initializer's completed root to be opened.
pub(super) fn collision(cause: &io::Error) -> bool {
    matches!(native_code(cause), Some(80 | 183))
}

#[cfg(test)]
mod error_tests;
