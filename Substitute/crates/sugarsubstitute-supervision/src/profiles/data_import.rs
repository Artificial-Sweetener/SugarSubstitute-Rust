// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit a closed historical browser profile without copying native lifetime authority.
use super::{locations, record};
use crate::durable_storage::{
    check,
    files::{self, Directory, ExistingAccess},
    invalid,
    protection::{self, Protection},
};
use std::{fs::File, io};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    lifecycle::NamespaceId,
};

/// Original exclusions and identity rows remain pinned until the copy transaction returns.
pub(crate) struct ReleasedProfileSource<'a> {
    directory: &'a Directory,
    controls: Vec<String>,
    namespace: NamespaceId,
    _pins: Vec<File>,
    _memory: MemoryClaim,
}
impl ReleasedProfileSource<'_> {
    /// Borrow the source only while its actual writer/witness exclusions remain held.
    pub(crate) fn directory(&self) -> &Directory {
        self.directory
    }
    /// Exclude exact root lifecycle names; nested authored names remain ordinary data.
    pub(crate) fn excluded_root_controls(&self) -> Vec<&str> {
        self.controls.iter().map(String::as_str).collect()
    }
    /// The original run must belong to a verified native generation of this installation.
    pub(crate) fn namespace(&self) -> NamespaceId {
        self.namespace
    }
}

/// Copy only after the canonical Released row and independent native exclusions agree.
/// `namespaces` comes from actual pinned generations named by protected installation selection.
/// No copied row can admit a new profile, process family or claim namespace.
/// # Errors
/// Preserves native/control causes; missing/held/foreign/corrupt controls and unknown reserved
/// lifecycle names reject without repairing or deleting any original.
pub(crate) fn with_released<T>(
    directory: &Directory,
    namespaces: &[NamespaceId],
    context: &JobContext,
    operation: impl FnOnce(&ReleasedProfileSource<'_>) -> io::Result<T>,
) -> io::Result<T> {
    check(context)?;
    let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
    protection::with(context, |owner| {
        owner.directory(
            directory.path(),
            crate::durable_storage::protected::directories::Mode::Existing,
            context,
        )?;
        let mut pins = vec![exclusion(directory, locations::CLAIM, owner, context)?];
        let current = row(directory, locations::RECORD, Shape::Current, owner, context)?;
        verify_store(current.record, directory, owner)?;
        if current.record.state != record::State::Released
            || !namespaces.contains(&current.record.run.owner.namespace)
        {
            return Err(invalid("Historical profile has no verified Released installation owner"));
        }
        pins.push(current.file);
        let mut controls = vec![locations::CLAIM.to_owned(), locations::RECORD.to_owned()];
        let custody = if current.custodied {
            Some(exclusion(directory, locations::CUSTODY, owner, context)?)
        } else {
            optional_exclusion(directory, locations::CUSTODY, owner, context)?
        };
        if let Some(file) = custody {
            pins.push(file);
            controls.push(locations::CUSTODY.to_owned());
        }
        let original_claim =
            optional_exclusion(directory, locations::ORIGINAL_CLAIM, owner, context)?;
        let original_record = optional_row(directory, locations::ORIGINAL_RECORD, owner, context)?;
        match (original_claim, original_record) {
            (Some(claim), Some(original)) => {
                verify_store(original.record, directory, owner)?;
                pins.extend([claim, original.file]);
                controls.extend([
                    locations::ORIGINAL_CLAIM.to_owned(),
                    locations::ORIGINAL_RECORD.to_owned(),
                ]);
            }
            (None, None) => {}
            _ => return Err(invalid("Historical profile backup controls disagree")),
        }
        let reserved = reserved_names(directory, context)?;
        for name in &reserved {
            if !name.starts_with(locations::RECOVERED_PREFIX) || !name.ends_with(".record") {
                continue;
            }
            let recovered = row(directory, name, Shape::Recovered, owner, context)?;
            verify_store(recovered.record, directory, owner)?;
            let stem = locations::recovered(recovered.record.run);
            if name != &format!("{stem}.record") {
                return Err(invalid("Historical recovered profile name differs from its run"));
            }
            let claim_name = format!("{stem}.claim");
            let claim = exclusion(directory, &claim_name, owner, context)?;
            pins.extend([recovered.file, claim]);
            controls.extend([name.clone(), claim_name]);
        }
        for name in reserved {
            if controls.iter().filter(|control| control.eq_ignore_ascii_case(&name)).count() != 1 {
                return Err(invalid("Unknown historical profile lifecycle control is preserved"));
            }
        }
        let source = ReleasedProfileSource {
            directory,
            controls,
            namespace: current.record.run.owner.namespace,
            _pins: pins,
            _memory: memory,
        };
        let result = operation(&source)?;
        check(context)?;
        Ok(result)
    })
}

fn exclusion(
    directory: &Directory,
    name: &str,
    owner: &Protection,
    context: &JobContext,
) -> io::Result<File> {
    let file = directory.open_existing(name, ExistingAccess::ExclusiveRead, context)?;
    files::single_link(&file)?;
    owner.verify_file(&file)?;
    if files::length(&file, context)? != 0 {
        return Err(invalid("Historical profile exclusion is not empty"));
    }
    Ok(file)
}
fn optional_exclusion(
    directory: &Directory,
    name: &str,
    owner: &Protection,
    context: &JobContext,
) -> io::Result<Option<File>> {
    match exclusion(directory, name, owner, context) {
        Ok(file) => Ok(Some(file)),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(cause),
    }
}
/// Lifecycle filenames bind the exact producer codec/state, never just a broadly decodable row.
enum Shape {
    Current,
    Original,
    Recovered,
}
struct ProfileRow {
    record: record::Record,
    custodied: bool,
    file: File,
}
fn row(
    directory: &Directory,
    name: &str,
    shape: Shape,
    owner: &Protection,
    context: &JobContext,
) -> io::Result<ProfileRow> {
    let mut file = directory.read(name)?;
    files::single_link(&file)?;
    owner.verify_file(&file)?;
    let bytes = files::read_admitted(&mut file, record::RECORD_BYTES, context)?;
    let custodied = bytes.get(8..10) == Some(&[2, 0]);
    let record = match shape {
        Shape::Original => record::Record::decode(&bytes),
        Shape::Recovered => {
            record::custody::CustodiedRecord::decode(&bytes).map(|row| row.original)
        }
        Shape::Current if custodied => {
            record::custody::CustodiedRecord::decode(&bytes).map(|row| row.original)
        }
        Shape::Current => record::Record::decode(&bytes),
    }
    .map_err(io::Error::other)?;
    if (matches!(shape, Shape::Original) && record.state != record::State::Released)
        || (matches!(shape, Shape::Recovered) && record.state != record::State::Held)
    {
        return Err(invalid("Historical profile lifecycle row differs from its producer state"));
    }
    Ok(ProfileRow { record, custodied, file })
}
fn optional_row(
    directory: &Directory,
    name: &str,
    owner: &Protection,
    context: &JobContext,
) -> io::Result<Option<ProfileRow>> {
    match row(directory, name, Shape::Original, owner, context) {
        Ok(row) => Ok(Some(row)),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(cause),
    }
}
fn verify_store(row: record::Record, directory: &Directory, owner: &Protection) -> io::Result<()> {
    if row.account != record::account_identity(owner.account_sid())
        || row.folder != directory.native_identity()?
    {
        return Err(invalid("Historical profile account or native directory association changed"));
    }
    Ok(())
}
fn reserved_names(directory: &Directory, context: &JobContext) -> io::Result<Vec<String>> {
    let mut reserved = Vec::new();
    let mut visited = 0;
    for entry in directory.entries(context)? {
        let entry = entry?;
        visited += 1;
        if visited > 8192 {
            return Err(invalid("Historical profile root inventory exceeds admission"));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("Historical profile name is not Unicode"))?;
        if !name
            .get(..locations::CONTROL_PREFIX.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(locations::CONTROL_PREFIX))
        {
            continue;
        }
        if reserved.len() >= 256 || name.len() > 1024 {
            return Err(invalid("Historical profile lifecycle inventory exceeds admission"));
        }
        if reserved.iter().any(|prior: &String| prior.eq_ignore_ascii_case(&name)) {
            return Err(invalid("Historical profile has ambiguous lifecycle-name aliases"));
        }
        reserved.try_reserve(1).map_err(io::Error::other)?;
        reserved.push(name);
    }
    Ok(reserved)
}

#[cfg(test)]
mod tests;
