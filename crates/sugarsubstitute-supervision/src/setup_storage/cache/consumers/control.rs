// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded native-reader controls against exact entry and control-directory identities.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;
const DIRECTORY: &str = "consumers";
pub(super) const REFERENCE_BYTES: usize = 84;

pub(super) struct Reference {
    pub(super) name: String,
    pub(super) identity: (u32, u32, u32),
    pub(super) family: CacheFamilyId,
}
pub(super) struct Slot {
    pub(super) directory: Directory,
    pub(super) references: Vec<Reference>,
}
/// Complete controls before publishing a fresh entry, using the caller's native protection grant.
pub(in crate::setup_storage::cache) fn initialize(
    directory: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<(u32, u32, u32)> {
    let path = directory.path().join(DIRECTORY);
    protection.directory(&path, Mode::CreateNew, context)?;
    let slot = Directory::open(&path, false)?;
    let identity = slot.identity()?;
    let mut owner = protection.file(&path.join("owner"), context)?;
    owner.write_all(&owner_record(directory.identity()?, identity))?;
    files::sync(&owner, context)?;
    Ok(identity)
}
/// Validate controls under the caller's existing protection budget. Read-only lookup may skip
/// a sharing conflict while another reader publishes/retires a reference; cleanup cannot.
pub(super) fn inspect(
    directory: &Directory,
    record: &record::Record,
    protection: &Protection,
    context: &JobContext,
    readonly_lookup: bool,
) -> io::Result<Option<Slot>> {
    let Some(identity) = record.consumers else {
        return Ok(None);
    };
    let slot = match Directory::open(&directory.path().join(DIRECTORY), false) {
        Ok(slot) => slot,
        Err(cause) if record.deleting && record::kind(&cause) == io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(cause) => return Err(cause),
    };
    if slot.identity()? != identity {
        return Err(invalid("Cache consumer directory identity changed"));
    }
    match slot.read("owner") {
        Ok(mut owner) => {
            protection.verify_file(&owner)?;
            if files::read_bounded(&mut owner, 64)? != owner_record(record.identity, identity) {
                return Err(invalid("Cache consumer ownership is damaged"));
            }
        }
        Err(cause) if record.deleting && record::kind(&cause) == io::ErrorKind::NotFound => {
            if slot.entries(context)?.next().transpose()?.is_some() {
                return Err(invalid("Cache consumer ownership was removed before its references"));
            }
        }
        Err(cause) => return Err(cause),
    }
    let mut references = Vec::new();
    let mut children = 0;
    for child in slot.entries(context)? {
        check(context)?;
        let name = child?
            .file_name()
            .to_str()
            .ok_or_else(|| invalid("Cache consumer filename is not Unicode"))?
            .to_owned();
        if name == "owner" {
            continue;
        }
        children += 1;
        if children > 2 * LIMIT {
            return Err(invalid("Cache consumer inventory is full"));
        }
        let pending = name.ends_with(".pin.next");
        let family = CacheFamilyId::parse(
            name.strip_suffix(if pending { ".pin.next" } else { ".pin" })
                .ok_or_else(|| invalid("Unknown cache consumer control"))?,
        )?;
        let mut file = match slot.read(&name) {
            Ok(file) => file,
            Err(cause) if readonly_lookup && record::busy(&cause) => continue,
            Err(cause) if record::kind(&cause) == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(cause),
        };
        protection.verify_file(&file)?;
        files::single_link(&file)?;
        if files::length(&file, context)? > REFERENCE_BYTES as u64 {
            return Err(invalid("Cache consumer control exceeds its schema bound"));
        }
        if !pending
            && files::read_bounded(&mut file, REFERENCE_BYTES)?
                != reference(record.identity, family)
        {
            return Err(invalid("Cache native consumer reference is damaged"));
        }
        references.push(Reference { name, identity: files::file_identity(&file)?, family });
    }
    Ok(Some(Slot { directory: slot, references }))
}
/// Bind ownership to exact native directories; checksums detect accidental corruption.
fn owner_record(entry: (u32, u32, u32), slot: (u32, u32, u32)) -> Vec<u8> {
    let mut bytes = b"SSCPINS1".to_vec();
    for word in [entry.0, entry.1, entry.2, slot.0, slot.1, slot.2] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    bytes
}
/// Persist canonical native matching facts independently of paths and Debug formatting.
pub(super) fn reference(entry: (u32, u32, u32), family: CacheFamilyId) -> Vec<u8> {
    let mut bytes = b"SSCPREF1".to_vec();
    for word in [entry.0, entry.1, entry.2] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(family.encoded().as_bytes());
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    bytes
}
