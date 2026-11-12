// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe AMD64 PE32+ and its sole bounded RSDS record without loading an image.
use super::*;
#[derive(Clone, Copy, Default)]
struct Section {
    address: u32,
    virtual_size: u32,
    raw: u32,
    raw_size: u32,
}

/// Read actual PE matching facts from the same whole-file-validated source.
/// # Errors
/// Rejects unknown image types, malformed ranges, overlapping RVA resolution,
/// missing/ambiguous RSDS records and preserved source errors.
pub(in super::super) fn read_pe<R: ReadAt>(
    source: &mut R,
) -> Result<PeIdentity, ImageError<R::Error>> {
    let dos = bytes::<64, _>(source, 0)?;
    if &dos[..2] != b"MZ" {
        return Err(ImageError::Invalid("dos_magic"));
    }
    let offset = u64::from(u32_at(&dos, 60)?);
    let header = bytes::<24, _>(source, offset)?;
    if &header[..4] != b"PE\0\0" || u16_at(&header, 4)? != 0x8664 {
        return Err(ImageError::Invalid("pe_machine"));
    }
    let count = usize::from(u16_at(&header, 6)?);
    let optional = u16_at(&header, 20)?;
    let flags = u16_at(&header, 22)?;
    if !(1..=96).contains(&count) || !(240..=4096).contains(&optional) || flags & 2 == 0 {
        return Err(ImageError::Invalid("pe_header_bound"));
    }
    let opt = bytes::<240, _>(source, offset + 24)?;
    if u16_at(&opt, 0)? != 0x20b || !(7..=16).contains(&u32_at(&opt, 108)?) {
        return Err(ImageError::Invalid("pe_optional_schema"));
    }
    let image_size = u32_at(&opt, 56)?;
    let headers = u32_at(&opt, 60)?;
    let table = offset + 24 + u64::from(optional);
    if image_size == 0
        || u64::from(headers) > source.length()
        || table + count as u64 * 40 > u64::from(headers)
    {
        return Err(ImageError::Invalid("pe_header_range"));
    }
    let mut sections = [Section::default(); 96];
    for (index, section) in sections[..count].iter_mut().enumerate() {
        let raw = bytes::<40, _>(source, table + index as u64 * 40)?;
        *section = Section {
            virtual_size: u32_at(&raw, 8)?,
            address: u32_at(&raw, 12)?,
            raw_size: u32_at(&raw, 16)?,
            raw: u32_at(&raw, 20)?,
        };
        if u64::from(section.raw) + u64::from(section.raw_size) > source.length()
            || u64::from(section.address) + u64::from(section.virtual_size.max(section.raw_size))
                > u64::from(image_size)
        {
            return Err(ImageError::Invalid("pe_section_range"));
        }
    }
    let debug_rva = u32_at(&opt, 160)?;
    let debug_size = u32_at(&opt, 164)?;
    if debug_rva == 0 || debug_size == 0 || debug_size > 28 * 16 || !debug_size.is_multiple_of(28) {
        return Err(ImageError::Invalid("pe_debug_bound"));
    }
    let debug = map(&sections[..count], headers, debug_rva, debug_size)?;
    let mut pdb = None;
    for index in 0..debug_size / 28 {
        let record = bytes::<28, _>(source, debug + u64::from(index) * 28)?;
        if u32_at(&record, 12)? != 2 {
            continue;
        }
        if pdb.is_some() {
            return Err(ImageError::Invalid("pe_ambiguous_codeview"));
        }
        let size = u32_at(&record, 16)?;
        let rva = u32_at(&record, 20)?;
        let raw = u32_at(&record, 24)?;
        if !(25..=4096).contains(&size)
            || map(&sections[..count], headers, rva, size)? != u64::from(raw)
            || u64::from(raw) + u64::from(size) > source.length()
        {
            return Err(ImageError::Invalid("pe_codeview_range"));
        }
        let rsds = bytes::<24, _>(source, u64::from(raw))?;
        if &rsds[..4] != b"RSDS" {
            return Err(ImageError::Invalid("pe_codeview_schema"));
        }
        let mut name = [0; 4096];
        let name_length = size as usize - 24;
        source.read_at(u64::from(raw) + 24, &mut name[..name_length]).map_err(ImageError::Read)?;
        if name[0] == 0
            || name[..name_length].last() != Some(&0)
            || name[..name_length - 1].contains(&0)
        {
            return Err(ImageError::Invalid("pe_codeview_name"));
        }
        let guid = number(&rsds, 4)?;
        let age = u32_at(&rsds, 20)?;
        if guid == [0; 16] || age == 0 {
            return Err(ImageError::Invalid("pe_codeview_identity"));
        }
        pdb = Some(PdbIdentity { guid, age });
    }
    Ok(PeIdentity {
        kind: if flags & 0x2000 != 0 { PeKind::Library } else { PeKind::Executable },
        image_size,
        timestamp: u32_at(&header, 8)?,
        checksum: u32_at(&opt, 64)?,
        pdb: pdb.ok_or(ImageError::Invalid("pe_missing_codeview"))?,
    })
}
fn map<E>(
    sections: &[Section],
    headers: u32,
    address: u32,
    length: u32,
) -> Result<u64, ImageError<E>> {
    let end = u64::from(address) + u64::from(length);
    let mut selected = (end <= u64::from(headers)).then_some(u64::from(address));
    for section in sections {
        if address >= section.address
            && end <= u64::from(section.address) + u64::from(section.raw_size)
        {
            let raw = u64::from(section.raw) + u64::from(address - section.address);
            if selected.replace(raw).is_some() {
                return Err(ImageError::Invalid("pe_ambiguous_rva"));
            }
        }
    }
    selected.ok_or(ImageError::Invalid("pe_unmapped_rva"))
}
