// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the small Windows self-relative owner/protected-DACL/medium-label contract.
#[cfg(test)]
mod tests;
pub(super) const MAXIMUM_BYTES: usize = 8192;
/// Refusals meaning the security no longer keeps other accounts out, as opposed to a malformed
/// or oversized descriptor.
pub(super) const UNTRUSTED: [&str; 3] =
    ["foreign_storage_security", "security_descriptor_shape", UNPROTECTED];
/// Refusal of a DACL that inherits again but still lets only this account and SYSTEM in, so
/// protecting it again restores the security this account set.
pub(super) const UNPROTECTED: &str = "unprotected_storage_security";
const CONTROL: u16 = 0x8000 | 0x1000 | 0x0004 | 0x0010;
pub(super) const SYSTEM: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
const MEDIUM: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 16, 0, 32, 0, 0];
pub(super) fn expected(sid: &[u8]) -> Result<Vec<u8>, &'static str> {
    expected_rights(sid, 0x001f01ff, 3)
}
/// Restrict a real recovery containment Job to its account/SYSTEM with no child ACL inheritance.
pub(super) fn expected_recovery_job(sid: &[u8]) -> Result<Vec<u8>, &'static str> {
    expected_rights(sid, 0x001f003f, 0)
}
fn expected_rights(sid: &[u8], rights: u32, inheritance: u8) -> Result<Vec<u8>, &'static str> {
    if sid.len() < 8
        || sid.len() > 68
        || sid[0] != 1
        || sid[1] > 15
        || sid.len() != 8 + 4 * sid[1] as usize
    {
        return Err("sid_shape");
    }
    let dacl = acl(&[ace(0, inheritance, rights, sid), ace(0, inheritance, rights, SYSTEM)]);
    let sacl = acl(&[ace(17, 0, 1, MEDIUM)]);
    let mut bytes = vec![0; 20];
    bytes[0] = 1;
    bytes[2..4].copy_from_slice(&CONTROL.to_le_bytes());
    for (offset, part) in [(4, sid), (16, dacl.as_slice()), (12, sacl.as_slice())] {
        let position = bytes.len() as u32;
        bytes[offset..offset + 4].copy_from_slice(&position.to_le_bytes());
        bytes.extend_from_slice(part);
    }
    Ok(bytes)
}
pub(super) fn ace(kind: u8, flags: u8, mask: u32, sid: &[u8]) -> Vec<u8> {
    let mut bytes = vec![kind, flags];
    bytes.extend_from_slice(&((8 + sid.len()) as u16).to_le_bytes());
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes.extend_from_slice(sid);
    bytes
}
pub(super) fn acl(aces: &[Vec<u8>]) -> Vec<u8> {
    let size = 8 + aces.iter().map(Vec::len).sum::<usize>();
    let mut bytes = vec![2, 0];
    bytes.extend_from_slice(&(size as u16).to_le_bytes());
    bytes.extend_from_slice(&(aces.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    for ace in aces {
        bytes.extend_from_slice(ace);
    }
    bytes
}
fn short(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    let value = bytes.get(offset..offset + 2).ok_or("security_range")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn offset(bytes: &[u8], field: usize) -> Result<usize, &'static str> {
    let value = bytes.get(field..field + 4).ok_or("security_range")?;
    let offset = u32::from_le_bytes([value[0], value[1], value[2], value[3]]) as usize;
    if offset < 20 || !offset.is_multiple_of(4) || offset >= bytes.len() {
        return Err("security_offset");
    }
    Ok(offset)
}
pub(super) fn owner(bytes: &[u8]) -> Result<&[u8], &'static str> {
    let offset = offset(bytes, 4)?;
    let header = bytes.get(offset..offset + 8).ok_or("security_owner")?;
    if header[0] != 1 || header[1] > 15 {
        return Err("security_owner");
    }
    bytes.get(offset..offset + 8 + 4 * header[1] as usize).ok_or("security_owner")
}
fn compare_acl(actual: &[u8], expected: &[u8], field: usize) -> Result<(), &'static str> {
    let start = offset(actual, field)?;
    let wanted = offset(expected, field)?;
    let size = short(actual, start + 2)? as usize;
    let limit =
        start.checked_add(size).filter(|end| *end <= actual.len()).ok_or("security_acl_bound")?;
    if size < 8
        || !matches!(actual[start], 2 | 4)
        || short(actual, start + 4)? != short(expected, wanted + 4)?
    {
        return Err("foreign_storage_security");
    }
    let count = short(expected, wanted + 4)?;
    let mut cursor = start + 8;
    let mut desired = wanted + 8;
    for _ in 0..count {
        let size = short(actual, cursor + 2)? as usize;
        let wanted_size = short(expected, desired + 2)? as usize;
        if size < 8 || !size.is_multiple_of(4) || size != wanted_size || cursor + size > limit {
            return Err("foreign_storage_security");
        }
        if actual.get(cursor..cursor + size) != expected.get(desired..desired + wanted_size) {
            return Err("foreign_storage_security");
        }
        cursor += size;
        desired += wanted_size;
    }
    Ok(())
}
/// Access an object grants or denies through one ACE.
struct Ace<'a> {
    kind: u8,
    flags: u8,
    mask: u32,
    sid: &'a [u8],
}

const ACCESS_ALLOWED: u8 = 0;
const ACCESS_DENIED: u8 = 1;
const MANDATORY_LABEL: u8 = 17;
const INHERIT_ONLY: u8 = 0x08;
const FILE_ALL_ACCESS: u32 = 0x001f01ff;
const GENERIC_ALL: u32 = 0x1000_0000;
const NO_WRITE_UP: u32 = 1;
const SELF_RELATIVE: u16 = 0x8000;
const DACL_PROTECTED: u16 = 0x1000;
const SACL_PRESENT: u16 = 0x0010;
const DACL_PRESENT: u16 = 0x0004;
/// Mandatory label authority (S-1-16) and the medium integrity level.
const LABEL_AUTHORITY: [u8; 6] = [0, 0, 0, 0, 0, 16];
const MEDIUM_LEVEL: u32 = 0x2000;

/// Every ACE of the ACL at `field`, or `None` when the descriptor has no such ACL.
fn aces(bytes: &[u8], field: usize) -> Result<Option<Vec<Ace<'_>>>, &'static str> {
    let at = bytes.get(field..field + 4).ok_or("security_range")?;
    if at == [0; 4] {
        return Ok(None);
    }
    let start = offset(bytes, field)?;
    let size = short(bytes, start + 2)? as usize;
    let limit =
        start.checked_add(size).filter(|end| *end <= bytes.len()).ok_or("security_acl_bound")?;
    if size < 8 || !matches!(bytes[start], 2 | 4) {
        return Err("foreign_storage_security");
    }
    let mut cursor = start + 8;
    let mut entries = Vec::new();
    for _ in 0..short(bytes, start + 4)? {
        let size = short(bytes, cursor + 2)? as usize;
        if size < 16 || !size.is_multiple_of(4) || cursor + size > limit {
            return Err("foreign_storage_security");
        }
        let entry = &bytes[cursor..cursor + size];
        let sid = &entry[8..];
        if sid[0] != 1 || sid[1] > 15 || sid.len() < 8 + 4 * sid[1] as usize {
            return Err("foreign_storage_security");
        }
        entries.push(Ace {
            kind: entry[0],
            flags: entry[1],
            mask: u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]),
            sid: &sid[..8 + 4 * sid[1] as usize],
        });
        cursor += size;
    }
    Ok(Some(entries))
}

/// Trust storage whose security still lets only this account and SYSTEM change it.
///
/// The owner is the account, the DACL is protected from inheritance and present, every ACE that
/// grants access names the account or SYSTEM, the account's grants cover full file access (a
/// generic-all grant does), and any integrity label is medium or higher with no write up.
/// Denials only take access away, so they are accepted. ACE order, inheritance flags and the
/// auto-inherited control bits do not change who can write the object and are accepted too:
/// permission tools, antivirus and backup software rewrite them while restoring the same access.
/// A DACL that meets all of this but inherits again is refused as [`UNPROTECTED`], which
/// [`super::reprotect`] repairs in place.
pub(super) fn verify(bytes: &[u8], sid: &[u8]) -> Result<(), &'static str> {
    expected(sid)?;
    if !(20..=MAXIMUM_BYTES).contains(&bytes.len()) || bytes[0] != 1 {
        return Err("security_descriptor_shape");
    }
    let control = short(bytes, 2)?;
    let required = SELF_RELATIVE | DACL_PRESENT;
    if control & required != required {
        return Err("security_descriptor_shape");
    }
    if owner(bytes)? != sid {
        return Err("foreign_storage_security");
    }
    let mut account = 0;
    for ace in aces(bytes, 16)?.ok_or("foreign_storage_security")? {
        match ace.kind {
            ACCESS_DENIED => {}
            ACCESS_ALLOWED if ace.sid == sid => {
                if ace.flags & INHERIT_ONLY == 0 {
                    // Generic all means every right of the object, file access included.
                    account |= if ace.mask & GENERIC_ALL != 0 { FILE_ALL_ACCESS } else { ace.mask };
                }
            }
            ACCESS_ALLOWED if ace.sid == SYSTEM => {}
            _ => return Err("foreign_storage_security"),
        }
    }
    if account & FILE_ALL_ACCESS != FILE_ALL_ACCESS {
        return Err("foreign_storage_security");
    }
    if control & SACL_PRESENT != 0 {
        for ace in aces(bytes, 12)?.unwrap_or_default() {
            let level =
                ace.sid.get(8..12).map(|rid| u32::from_le_bytes([rid[0], rid[1], rid[2], rid[3]]));
            if ace.kind != MANDATORY_LABEL
                || ace.sid[1] != 1
                || ace.sid[2..8] != LABEL_AUTHORITY
                || level.is_none_or(|level| level < MEDIUM_LEVEL)
                || ace.mask & NO_WRITE_UP == 0
            {
                return Err("foreign_storage_security");
            }
        }
    }
    if control & DACL_PROTECTED == 0 {
        return Err(UNPROTECTED);
    }
    Ok(())
}
/// Require the exact canonical descriptor and closed rights policy this process gave the Job;
/// no tool outside the process can reach a Job's security.
pub(super) fn verify_recovery_job(bytes: &[u8], sid: &[u8]) -> Result<(), &'static str> {
    verify_expected(bytes, sid, expected_recovery_job(sid)?)
}
fn verify_expected(bytes: &[u8], sid: &[u8], expected: Vec<u8>) -> Result<(), &'static str> {
    if !(20..=MAXIMUM_BYTES).contains(&bytes.len())
        || bytes[0] != 1
        || short(bytes, 2)? & CONTROL != CONTROL
    {
        return Err("security_descriptor_shape");
    }
    if owner(bytes)? != sid {
        return Err("foreign_storage_security");
    }
    compare_acl(bytes, &expected, 16)?;
    compare_acl(bytes, &expected, 12)
}
