// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read terminal historical claims under their original writer and independent witness exclusions.
//! No historical record is published, upgraded, retired or admitted as a live ownership claim.
use super::{Record, RecordState};
use crate::durable_storage::{
    check,
    files::{self, Directory, ExistingAccess},
    invalid,
    protected::directories::Mode,
    protection,
};
use std::{fs::File, io};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    lifecycle::NamespaceId,
};
#[cfg(test)]
mod tests;

/// Keep selected original records and exclusions alive through all dependent legacy reads.
pub(crate) struct HistoricalClaims {
    _root: Directory,
    _directories: Vec<Directory>,
    _files: Vec<File>,
    _memory: MemoryClaim,
}

/// Pin only claims whose decoded owner belongs to complete protected selected generations.
/// The caller derives `root` from the actual account known-folder reader, never environment text.
/// # Errors
/// Rejects active/nonterminal claims, missing independent witnesses, changed native protection,
/// unsupported record bytes and bounded-inventory overflow. No missing control is created.
pub(crate) fn released(
    root: Directory,
    namespaces: &[NamespaceId],
    context: &JobContext,
) -> io::Result<HistoricalClaims> {
    check(context)?;
    if namespaces.is_empty() || namespaces.len() > 3 {
        return Err(invalid("Historical claim namespaces exceed selected generation admission"));
    }
    let memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    protection::with(context, |owner| {
        owner.directory(root.path(), Mode::Existing, context)?;
        let coordinators = root.child("coordinators")?.retained()?;
        let backends = root.child("backends")?.retained()?;
        for directory in [&coordinators, &backends] {
            owner.directory(directory.path(), Mode::Existing, context)?;
        }
        let mut pins = Vec::new();
        for namespace in namespaces {
            let stem =
                super::super::durable_storage::protected::record_file::hex(namespace.bytes());
            let record = optional_record(&coordinators, &stem, owner, context)?;
            match record {
                Some((record, file)) => {
                    if record.resource != 0 || record.owner.namespace != *namespace {
                        return Err(invalid(
                            "Historical coordinator namespace association differs",
                        ));
                    }
                    terminal(&record)?;
                    pins.push(exclusion(&coordinators, &format!("{stem}.claim"), owner, context)?);
                    match coordinators.open_existing(
                        &format!("{stem}.entry.claim"),
                        ExistingAccess::ExclusiveRead,
                        context,
                    ) {
                        Ok(file) => {
                            verify_exclusion(&file, owner, context)?;
                            pins.push(file);
                        }
                        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                        Err(cause) => return Err(cause),
                    }
                    pins.push(file);
                }
                None => match coordinators.read(&format!("{stem}.claim")) {
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                    Err(cause) => return Err(cause),
                    Ok(_) => return Err(invalid("Historical coordinator controls disagree")),
                },
            }
        }
        let mut visited = 0;
        let mut names = 0_usize;
        for entry in backends.entries(context)? {
            let entry = entry?;
            visited += 1;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Historical backend name is not Unicode"))?;
            names = names
                .checked_add(name.capacity() + 128)
                .ok_or_else(|| invalid("Historical backend inventory overflow"))?;
            if visited > 8192 || names > 1024 * 1024 {
                return Err(invalid("Historical backend inventory exceeds admission"));
            }
            let Some(stem) = name.strip_suffix(".record").filter(|stem| digest(stem)) else {
                continue;
            };
            let Some((record, file)) = optional_record(&backends, stem, owner, context)? else {
                return Err(invalid("Historical backend disappeared during inventory"));
            };
            if !namespaces.contains(&record.owner.namespace) {
                continue;
            }
            if record.resource == 0
                || super::super::durable_storage::protected::record_file::hex(&record.digest)
                    != stem
            {
                return Err(invalid("Historical backend native record association differs"));
            }
            terminal(&record)?;
            pins.try_reserve(3).map_err(io::Error::other)?;
            pins.push(exclusion(&backends, &format!("{stem}.claim"), owner, context)?);
            match backends.open_existing(
                &format!("{stem}.custody"),
                ExistingAccess::ExclusiveRead,
                context,
            ) {
                Ok(file) => {
                    verify_exclusion(&file, owner, context)?;
                    pins.push(file);
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound && record.version == 1 => {}
                Err(cause) => return Err(cause),
            }
            pins.push(file);
        }
        check(context)?;
        Ok(HistoricalClaims {
            _root: root,
            _directories: vec![coordinators, backends],
            _files: pins,
            _memory: memory,
        })
    })
}
fn optional_record(
    directory: &Directory,
    stem: &str,
    owner: &protection::Protection,
    context: &JobContext,
) -> io::Result<Option<(Record, File)>> {
    let mut file = match directory.read(&format!("{stem}.record")) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    files::single_link(&file)?;
    owner.verify_file(&file)?;
    let bytes: [u8; 128] = files::read_admitted(&mut file, 128, context)?
        .try_into()
        .map_err(|_| invalid("Historical claim record has an incomplete fixed encoding"))?;
    let record = Record::decode(&bytes).map_err(io::Error::other)?;
    check(context)?;
    Ok(Some((record, file)))
}
fn terminal(record: &Record) -> io::Result<()> {
    if record.state != RecordState::Released || record.process != 0 {
        return Err(invalid("Historical claim has no terminal Released owner"));
    }
    Ok(())
}
fn exclusion(
    directory: &Directory,
    name: &str,
    owner: &protection::Protection,
    context: &JobContext,
) -> io::Result<File> {
    let file = directory.open_existing(name, ExistingAccess::ExclusiveRead, context)?;
    verify_exclusion(&file, owner, context)?;
    Ok(file)
}
fn verify_exclusion(
    file: &File,
    owner: &protection::Protection,
    context: &JobContext,
) -> io::Result<()> {
    files::single_link(file)?;
    owner.verify_file(file)?;
    if files::length(file, context)? != 0 {
        return Err(invalid("Historical claim exclusion is not empty"));
    }
    Ok(())
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
