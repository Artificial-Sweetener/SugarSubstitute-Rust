// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream canonical text and genuine committed dump bytes into one receiving stage.
use super::{
    ReportExportCause as E, ReportExportCode as C,
    archive::{self, Crc, Entry, Sink},
};
use crate::{incident::IncidentDumpReadLease, report_view::ReportView};
use sha2::{Digest, Sha256};
use sugarsubstitute_supervision::ReportFile;
pub(super) const CHUNK: usize = 65_536;
/// Complete write observations retained until activation and checked retirement.
pub(super) struct Written {
    pub(super) bytes: u64,
    pub(super) sha256: [u8; 32],
}
/// Hash only complete observed writes to the same retained receiving-stage inode.
struct Writer<'a> {
    destination: &'a mut ReportFile,
    hash: Sha256,
    bytes: u64,
}
impl Sink for Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        for chunk in bytes.chunks(CHUNK) {
            let next = self
                .bytes
                .checked_add(chunk.len() as u64)
                .filter(|bytes| *bytes <= 268_435_456)
                .ok_or(E::Rejected(C::Bounds))?;
            self.destination.write(chunk)?;
            self.hash.update(chunk);
            self.bytes = next;
        }
        Ok(())
    }
    fn position(&self) -> u64 {
        self.bytes
    }
}
/// Stream exact canonical text and optional genuine same-inode native dump.
/// The original reader independently guards every bounded read and current Summary.
pub(super) fn write(
    destination: &mut ReportFile,
    view: &ReportView,
    dump: Option<&mut IncidentDumpReadLease>,
    buffer: &mut [u8; CHUNK],
) -> Result<Written, E> {
    let mut writer = Writer { destination, hash: Sha256::new(), bytes: 0 };
    let text = view.text().as_bytes();
    if let Some(dump) = dump {
        let facts = dump.facts().map_err(E::Dump)?;
        if facts.bytes() == 0 || archive::bundle_bytes(text.len() as u64, facts.bytes()).is_err() {
            return Err(E::Rejected(C::Bounds));
        }
        let offset = archive::header(&mut writer, archive::TEXT_NAME)?;
        let mut crc = Crc::new();
        crc.update(text);
        writer.put(text)?;
        let text_entry =
            Entry { offset, size: archive::count(text.len() as u64)?, crc: crc.finish() };
        archive::descriptor(&mut writer, text_entry)?;
        let offset = archive::header(&mut writer, archive::DUMP_NAME)?;
        let mut hash = Sha256::new();
        let mut crc = Crc::new();
        let mut position = 0;
        while position < facts.bytes() {
            let expected = (facts.bytes() - position).min(CHUNK as u64) as usize;
            let actual = dump.read_chunk(position, &mut buffer[..expected]).map_err(E::Dump)?;
            if actual != expected {
                return Err(E::Rejected(C::Bounds));
            }
            hash.update(&buffer[..actual]);
            crc.update(&buffer[..actual]);
            writer.put(&buffer[..actual])?;
            position += actual as u64;
        }
        if <[u8; 32]>::from(hash.finalize()) != facts.sha256() {
            return Err(E::Rejected(C::Identity));
        }
        dump.check_current().map_err(E::Dump)?;
        let dump_entry = Entry { offset, size: archive::count(position)?, crc: crc.finish() };
        archive::descriptor(&mut writer, dump_entry)?;
        archive::finish(&mut writer, [text_entry, dump_entry])?;
    } else {
        writer.put(text)?;
    }
    Ok(Written { bytes: writer.bytes, sha256: writer.hash.finalize().into() })
}
