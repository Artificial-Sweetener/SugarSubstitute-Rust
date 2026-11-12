// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode two stored ZIP entries with descriptors and fixed bounded metadata.
use super::{ReportExportCause as E, ReportExportCode as C};
pub(super) const TEXT_NAME: &[u8] = b"report.txt";
pub(super) const DUMP_NAME: &[u8] = b"native.dmp";
/// Internal byte sink; this does not create a native receiving-file capability.
pub(super) trait Sink {
    /// Append complete bytes or preserve the original destination rejection.
    fn put(&mut self, bytes: &[u8]) -> Result<(), E>;
    /// Return actual successfully appended bytes, independent of capacity.
    fn position(&self) -> u64;
}
/// Fixed ZIP directory facts, never a source or native-file capability.
#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub(super) offset: u32,
    pub(super) size: u32,
    pub(super) crc: u32,
}
/// Emit a stored entry with a streaming descriptor and return its exact offset.
pub(super) fn header(sink: &mut impl Sink, name: &[u8]) -> Result<u32, E> {
    let offset = count(sink.position())?;
    sink.put(&0x0403_4b50_u32.to_le_bytes())?;
    words(sink, &[20, 8, 0, 0, 33])?;
    dwords(sink, &[0, 0, 0])?;
    words(sink, &[name.len() as u16, 0])?;
    sink.put(name)?;
    Ok(offset)
}
/// Emit CRC and complete byte lengths after one successful entry stream.
pub(super) fn descriptor(sink: &mut impl Sink, entry: Entry) -> Result<(), E> {
    dwords(sink, &[0x0807_4b50, entry.crc, entry.size, entry.size])
}
/// Finish the two exact fixed entry names and their bounded central directory.
pub(super) fn finish(sink: &mut impl Sink, entries: [Entry; 2]) -> Result<(), E> {
    let start = count(sink.position())?;
    for (name, entry) in [TEXT_NAME, DUMP_NAME].into_iter().zip(entries) {
        dwords(sink, &[0x0201_4b50])?;
        words(sink, &[20, 20, 8, 0, 0, 33])?;
        dwords(sink, &[entry.crc, entry.size, entry.size])?;
        words(sink, &[name.len() as u16, 0, 0, 0, 0])?;
        dwords(sink, &[0, entry.offset])?;
        sink.put(name)?;
    }
    let size = count(sink.position())?.checked_sub(start).ok_or(E::Rejected(C::Bounds))?;
    dwords(sink, &[0x0605_4b50])?;
    words(sink, &[0, 0, 2, 2])?;
    dwords(sink, &[size, start])?;
    words(sink, &[0])
}
/// Reject ZIP64 sizes; admitted diagnostic exports always fit the ZIP32 contract.
pub(super) fn count(value: u64) -> Result<u32, E> {
    u32::try_from(value).map_err(|_| E::Rejected(C::Bounds))
}
/// Emit little-endian ZIP words without allocating metadata buffers.
fn words(sink: &mut impl Sink, values: &[u16]) -> Result<(), E> {
    for value in values {
        sink.put(&value.to_le_bytes())?;
    }
    Ok(())
}
/// Emit little-endian ZIP double words through the checked destination sink.
fn dwords(sink: &mut impl Sink, values: &[u32]) -> Result<(), E> {
    for value in values {
        sink.put(&value.to_le_bytes())?;
    }
    Ok(())
}
/// Incremental ZIP CRC32 state with no table allocation or compression library.
pub(super) struct Crc(u32);
impl Crc {
    /// Start the standard inverted CRC32 accumulator.
    pub(super) const fn new() -> Self {
        Self(u32::MAX)
    }
    /// Incorporate one bounded stream chunk without retaining source bytes.
    pub(super) fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u32::from(*byte);
            for _ in 0..8 {
                self.0 = (self.0 >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(self.0 & 1));
            }
        }
    }
    /// Return the standard final inverted CRC32 value.
    pub(super) const fn finish(self) -> u32 {
        !self.0
    }
}
#[cfg(test)]
mod tests;

/// Bound the full ZIP result, including both fixed names/descriptors/directory records.
/// The diagnostic result protocol admits at most256MiB including this246-byte envelope.
pub(super) fn bundle_bytes(text: u64, dump: u64) -> Result<u64, E> {
    text.checked_add(dump)
        .and_then(|total| total.checked_add(246))
        .filter(|total| *total <= 268_435_456)
        .ok_or(E::Rejected(C::Bounds))
}
