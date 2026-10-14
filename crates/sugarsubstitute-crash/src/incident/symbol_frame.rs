// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Extract one actual exception/module frame from the same protected committed inode.
use super::{DumpReadCode, IncidentDumpReadLease};
use sugarsubstitute_contracts::incident::{FailureEvidence, IncidentError, PdbIdentity};
/// Bounded frame-validation failure; actual read causes remain with the committed lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolFrameError {
    /// Actual protected read or original control failed.
    Source(DumpReadCode),
    /// Header, directory, record or RVA bounds are corrupt.
    Corrupt,
    /// Actual exception code/address differs from independently supplied native fault evidence.
    ExceptionMismatch,
    /// Fault address does not belong to the exact identity-bearing module.
    UnknownModule,
    /// Complete CodeView GUID/age are unavailable or malformed.
    CodeView,
}
/// Actual captured module/address association. Construction stays with this source owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapturedExceptionFrame {
    code: u32,
    address: u64,
    base: u64,
    size: u32,
    pdb: PdbIdentity,
}
impl CapturedExceptionFrame {
    /// Actual exception address in the captured process address space.
    pub const fn address(self) -> u64 {
        self.address
    }
    /// Actual identity-bearing module image base.
    pub const fn image_base(self) -> u64 {
        self.base
    }
    /// Actual module image size in bytes.
    pub const fn image_size(self) -> u32 {
        self.size
    }
    /// Actual exact CodeView GUID/age from the same module record.
    pub const fn pdb(self) -> PdbIdentity {
        self.pdb
    }
    /// Actual exception code from the same validated captured native record.
    pub const fn code(self) -> u32 {
        self.code
    }
}
trait Read {
    fn bytes(&mut self, offset: u64, output: &mut [u8]) -> Result<(), SymbolFrameError>;
}
impl Read for IncidentDumpReadLease {
    fn bytes(&mut self, offset: u64, output: &mut [u8]) -> Result<(), SymbolFrameError> {
        let count = self.read_chunk(offset, output).map_err(SymbolFrameError::Source)?;
        if count != output.len() {
            return Err(SymbolFrameError::Corrupt);
        }
        Ok(())
    }
}
impl IncidentDumpReadLease {
    /// Extract the actual failing frame after same-file subject/build/UUID validation.
    /// It never follows dump-supplied paths or uses them to select symbols.
    /// Original failure attribution is preserved; categorical or unexplained evidence
    /// supplies no competing native exception pair. Supplied native faults must match exactly.
    /// # Errors
    /// Rejects malformed RVAs, duplicate streams, foreign exception facts and wrong module.
    /// Original protected read/control failures stay in this same lease.
    pub fn exception_frame(&mut self) -> Result<CapturedExceptionFrame, SymbolFrameError> {
        let length = self.facts().map_err(SymbolFrameError::Source)?.bytes();
        let index = self.parser_identity().map_err(SymbolFrameError::Source)?.module_index();
        let failure = self.summary().map_err(SymbolFrameError::Source)?.summary().facts().failure();
        extract(self, length, index, failure)
    }
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
fn bounded(offset: u64, count: u64, length: u64) -> Result<(), SymbolFrameError> {
    if offset.checked_add(count).is_none_or(|end| end > length) {
        return Err(SymbolFrameError::Corrupt);
    }
    Ok(())
}
fn extract(
    source: &mut impl Read,
    length: u64,
    module_index: u32,
    failure: FailureEvidence,
) -> Result<CapturedExceptionFrame, SymbolFrameError> {
    let mut header = [0; 32];
    bounded(0, 32, length)?;
    source.bytes(0, &mut header)?;
    if header[..4] != *b"MDMP" || u32_at(&header, 4) & 0xffff != 0xa793 {
        return Err(SymbolFrameError::Corrupt);
    }
    let count = u32_at(&header, 8);
    let directory = u64::from(u32_at(&header, 12));
    if count == 0 || count > 4096 {
        return Err(SymbolFrameError::Corrupt);
    }
    bounded(directory, u64::from(count) * 12, length)?;
    let mut exception = None;
    let mut modules = None;
    for index in 0..count {
        let mut row = [0; 12];
        source.bytes(directory + u64::from(index) * 12, &mut row)?;
        let kind = u32_at(&row, 0);
        let size = u32_at(&row, 4);
        let offset = u64::from(u32_at(&row, 8));
        bounded(offset, u64::from(size), length)?;
        let target = match kind {
            6 => Some(&mut exception),
            4 => Some(&mut modules),
            _ => None,
        };
        if let Some(target) = target
            && target.replace((offset, size)).is_some()
        {
            return Err(SymbolFrameError::Corrupt);
        }
    }
    let (offset, size) = exception.ok_or(SymbolFrameError::Corrupt)?;
    if size < 168 {
        return Err(SymbolFrameError::Corrupt);
    }
    let mut record = [0; 40];
    source.bytes(offset, &mut record)?;
    let code = u32_at(&record, 8);
    let address = u64_at(&record, 24);
    if u32_at(&record, 32) > 15 {
        return Err(SymbolFrameError::Corrupt);
    }
    if let FailureEvidence::NativeFault { code: expected_code, address: expected_address } = failure
        && (expected_code != code || expected_address != address)
    {
        return Err(SymbolFrameError::ExceptionMismatch);
    }
    let (offset, size) = modules.ok_or(SymbolFrameError::Corrupt)?;
    if size < 4 {
        return Err(SymbolFrameError::Corrupt);
    }
    let mut module_count = [0; 4];
    source.bytes(offset, &mut module_count)?;
    let count = u32_at(&module_count, 0);
    if count == 0
        || count > 4096
        || module_index >= count
        || u64::from(size) < 4 + u64::from(count) * 108
    {
        return Err(SymbolFrameError::Corrupt);
    }
    let mut module = [0; 108];
    source.bytes(offset + 4 + u64::from(module_index) * 108, &mut module)?;
    let base = u64_at(&module, 0);
    let size = u32_at(&module, 8);
    let end = base.checked_add(u64::from(size)).ok_or(SymbolFrameError::UnknownModule)?;
    if size == 0 || address < base || address >= end {
        return Err(SymbolFrameError::UnknownModule);
    }
    let cv_size = u32_at(&module, 76);
    let cv = u64::from(u32_at(&module, 80));
    bounded(cv, u64::from(cv_size), length)?;
    if cv_size < 24 {
        return Err(SymbolFrameError::CodeView);
    }
    let mut codeview = [0; 24];
    source.bytes(cv, &mut codeview)?;
    if codeview[..4] != *b"RSDS" {
        return Err(SymbolFrameError::CodeView);
    }
    let mut guid = [0; 16];
    guid.copy_from_slice(&codeview[4..20]);
    let pdb = PdbIdentity::new(guid, u32_at(&codeview, 20))
        .map_err(|_: IncidentError| SymbolFrameError::CodeView)?;
    Ok(CapturedExceptionFrame { code, address, base, size, pdb })
}
#[cfg(test)]
mod tests;
