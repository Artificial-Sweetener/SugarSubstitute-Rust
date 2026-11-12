// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the small Windows self-relative owner/protected-DACL/medium-label contract.
#[cfg(test)]
mod tests;
pub(super) const MAXIMUM_BYTES: usize = 8192;
const CONTROL: u16 = 0x8000 | 0x1000 | 0x0004 | 0x0010;
const SYSTEM: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
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
fn ace(kind: u8, flags: u8, mask: u32, sid: &[u8]) -> Vec<u8> {
    let mut bytes = vec![kind, flags];
    bytes.extend_from_slice(&((8 + sid.len()) as u16).to_le_bytes());
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes.extend_from_slice(sid);
    bytes
}
fn acl(aces: &[Vec<u8>]) -> Vec<u8> {
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
fn owner(bytes: &[u8]) -> Result<&[u8], &'static str> {
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
pub(super) fn verify(bytes: &[u8], sid: &[u8]) -> Result<(), &'static str> {
    verify_expected(bytes, sid, expected(sid)?)
}
/// Require the same canonical descriptor structure and the exact closed Job rights policy.
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
