// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve only the bounded MSF7 PDB information header through actual block indirection.
use super::*;
const MAGIC: &[u8; 32] = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0";
/// Read actual PDB GUID/age without a symbol search, stack walk or full-file allocation.
/// # Errors
/// Rejects malformed MSF7 block maps and missing/unknown PDB information headers,
/// retaining any original external read error.
pub(in super::super) fn read_pdb<R: ReadAt>(
    source: &mut R,
) -> Result<PdbIdentity, ImageError<R::Error>> {
    let header = bytes::<56, _>(source, 0)?;
    if &header[..32] != MAGIC {
        return Err(ImageError::Invalid("pdb_msf_magic"));
    }
    let block = u32_at(&header, 32)?;
    let free = u32_at(&header, 36)?;
    let count = u32_at(&header, 40)?;
    let directory = u32_at(&header, 44)?;
    let map = u32_at(&header, 52)?;
    if ![512, 1024, 2048, 4096].contains(&block)
        || ![1, 2].contains(&free)
        || count < 4
        || u64::from(block) * u64::from(count) != source.length()
        || directory < 16
        || directory > block * (block / 4)
    {
        return Err(ImageError::Invalid("pdb_msf_bound"));
    }
    valid_block(map, block, count)?;
    let pages = directory.div_ceil(block);
    let mut table = [0; 4096];
    source
        .read_at(u64::from(map) * u64::from(block), &mut table[..pages as usize * 4])
        .map_err(ImageError::Read)?;
    for index in 0..pages as usize {
        let page = u32_at(&table, index * 4)?;
        valid_block(page, block, count)?;
        if page == map
            || (0..index)
                .any(|prior| u32_at::<R::Error>(&table, prior * 4).is_ok_and(|value| value == page))
        {
            return Err(ImageError::Invalid("pdb_directory_alias"));
        }
    }
    let streams = directory_u32(source, &table, block, directory, 0)?;
    if !(2..=65536).contains(&streams) || 4 + u64::from(streams) * 4 > u64::from(directory) {
        return Err(ImageError::Invalid("pdb_stream_bound"));
    }
    let first_size = directory_u32(source, &table, block, directory, 4)?;
    let info_size = directory_u32(source, &table, block, directory, 8)?;
    let first_pages = if first_size == u32::MAX { 0 } else { first_size.div_ceil(block) };
    if info_size < 28 || info_size == u32::MAX || info_size.div_ceil(block) > count {
        return Err(ImageError::Invalid("pdb_info_bound"));
    }
    let start = 4 + u64::from(streams) * 4 + u64::from(first_pages) * 4;
    let end = start + u64::from(info_size.div_ceil(block)) * 4;
    if end > u64::from(directory) {
        return Err(ImageError::Invalid("pdb_info_map_range"));
    }
    let page = directory_u32(source, &table, block, directory, start)?;
    valid_block(page, block, count)?;
    if page == map
        || (0..pages as usize)
            .any(|index| u32_at::<R::Error>(&table, index * 4).is_ok_and(|value| value == page))
    {
        return Err(ImageError::Invalid("pdb_info_alias"));
    }
    let info = bytes::<28, _>(source, u64::from(page) * u64::from(block))?;
    if ![20000404, 20030901, 20091201, 20140508].contains(&u32_at(&info, 0)?) {
        return Err(ImageError::Invalid("pdb_info_version"));
    }
    let age = u32_at(&info, 8)?;
    let guid = number(&info, 12)?;
    if age == 0 || guid == [0; 16] {
        return Err(ImageError::Invalid("pdb_info_identity"));
    }
    Ok(PdbIdentity { guid, age })
}
fn valid_block<E>(page: u32, block: u32, count: u32) -> Result<(), ImageError<E>> {
    if page >= count || page == 0 || matches!(page % block, 1 | 2) {
        Err(ImageError::Invalid("pdb_block_range"))
    } else {
        Ok(())
    }
}
fn directory_u32<R: ReadAt>(
    source: &mut R,
    table: &[u8],
    block: u32,
    length: u32,
    offset: u64,
) -> Result<u32, ImageError<R::Error>> {
    if offset + 4 > u64::from(length) || !offset.is_multiple_of(4) {
        return Err(ImageError::Invalid("pdb_directory_range"));
    }
    let logical = offset / u64::from(block);
    let page = u32_at(table, logical as usize * 4)?;
    let raw = u64::from(page) * u64::from(block) + offset % u64::from(block);
    Ok(u32::from_le_bytes(bytes::<4, _>(source, raw)?))
}
