// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound exact release associations and one failing-frame result, not an unwound stack.
use super::{BuildIdentity, DumpId, IncidentError};
use std::num::NonZeroU32;

/// Bounded module/source basename label; never a native path or lookup authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLabel(Box<str>);
impl FileLabel {
    /// Maximum retained validated UTF-8 bytes for this owned annotation.
    pub const MAX_BYTES: usize = 128;

    /// Preserve 1..=128 ASCII basename bytes with no path/control syntax.
    /// # Errors
    /// Rejects empty/oversized labels, dot-only names and non-label characters.
    pub fn new(value: &str) -> Result<Self, IncidentError> {
        if value.is_empty()
            || value.len() > Self::MAX_BYTES
            || matches!(value, "." | "..")
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(IncidentError::InvalidText);
        }
        Ok(Self(value.into()))
    }
    /// Borrow the exact display/matching label, which cannot grant filesystem access.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Bounded UTF-8 function symbol spelling supplied by the symbol owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolName(Box<str>);
impl SymbolName {
    /// Maximum retained validated UTF-8 bytes for this owned annotation.
    pub const MAX_BYTES: usize = 512;

    /// Preserve at most 512 UTF-8 bytes without embedded controls or empty content.
    /// # Errors
    /// Returns `InvalidText` for invalid length or control characters.
    pub fn new(value: &str) -> Result<Self, IncidentError> {
        if value.is_empty() || value.len() > Self::MAX_BYTES || value.chars().any(char::is_control)
        {
            return Err(IncidentError::InvalidText);
        }
        Ok(Self(value.into()))
    }
    /// Borrow original spelling; no localization/redaction is performed here.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Exact nonzero CodeView PDB GUID plus positive age, without loaded symbol handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PdbIdentity {
    guid: [u8; 16],
    age: NonZeroU32,
}
impl PdbIdentity {
    /// Validate required GUID/age shape; the adapter separately compares PE and PDB.
    /// # Errors
    /// Returns `InvalidIdentity` for a zero GUID or age.
    pub fn new(guid: [u8; 16], age: u32) -> Result<Self, IncidentError> {
        if guid == [0; 16] {
            return Err(IncidentError::InvalidIdentity);
        }
        Ok(Self { guid, age: NonZeroU32::new(age).ok_or(IncidentError::InvalidIdentity)? })
    }
    /// Borrow exact canonical GUID matching bytes.
    pub const fn guid(&self) -> &[u8; 16] {
        &self.guid
    }
    /// Return positive CodeView age.
    pub const fn age(self) -> u32 {
        self.age.get()
    }
}
/// Architecture of the associated image/symbol pair; no platform is qualified by this enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolArchitecture {
    /// 64-bit x86 image and PDB association used by the Windows contract.
    X86_64,
}
/// Exact release matching facts, independent of symbol storage and native lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseSymbols {
    build: BuildIdentity,
    module: FileLabel,
    architecture: SymbolArchitecture,
    pdb: PdbIdentity,
    image_sha256: [u8; 32],
    symbols_sha256: [u8; 32],
}
impl ReleaseSymbols {
    /// Retain bounded expected identity/digests; this performs no artifact verification.
    pub const fn new(
        build: BuildIdentity,
        module: FileLabel,
        architecture: SymbolArchitecture,
        pdb: PdbIdentity,
        image_sha256: [u8; 32],
        symbols_sha256: [u8; 32],
    ) -> Self {
        Self { build, module, architecture, pdb, image_sha256, symbols_sha256 }
    }
    /// Borrow the exact expected release build identity.
    pub const fn build(&self) -> &BuildIdentity {
        &self.build
    }
    /// Borrow the bounded module basename, never a dump-derived native lookup path.
    pub const fn module(&self) -> &FileLabel {
        &self.module
    }
    /// Return image architecture.
    pub const fn architecture(&self) -> SymbolArchitecture {
        self.architecture
    }
    /// Return the exact expected CodeView identity.
    pub const fn pdb(&self) -> PdbIdentity {
        self.pdb
    }
    /// Borrow expected image content digest.
    pub const fn image_sha256(&self) -> &[u8; 32] {
        &self.image_sha256
    }
    /// Borrow expected symbols content digest.
    pub const fn symbols_sha256(&self) -> &[u8; 32] {
        &self.symbols_sha256
    }
}
/// A source basename and positive line number, without exposing developer native paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLine {
    file: FileLabel,
    line: NonZeroU32,
}
impl SourceLine {
    /// Retain a source label only when an actual positive line was supplied.
    /// # Errors
    /// Returns `InvalidIdentity` for line zero; missing lines use `None` instead.
    pub fn new(file: FileLabel, line: u32) -> Result<Self, IncidentError> {
        Ok(Self { file, line: NonZeroU32::new(line).ok_or(IncidentError::InvalidIdentity)? })
    }
    /// Borrow the source basename label.
    pub const fn file(&self) -> &FileLabel {
        &self.file
    }
    /// Return the actual positive line number.
    pub const fn line(&self) -> u32 {
        self.line.get()
    }
}
/// One actual dump exception frame associated with exact release symbols.
/// Construction retains supplied facts; it does not authenticate a symbol helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailingFrame {
    dump: DumpId,
    release: ReleaseSymbols,
    address: u64,
    module_offset: u64,
    symbol: SymbolName,
    source: Option<SourceLine>,
}
impl FailingFrame {
    /// Retain the actual exception address/module offset and optional actual source line.
    pub const fn new(
        dump: DumpId,
        release: ReleaseSymbols,
        address: u64,
        module_offset: u64,
        symbol: SymbolName,
        source: Option<SourceLine>,
    ) -> Self {
        Self { dump, release, address, module_offset, symbol, source }
    }
    /// Return the exact finished dump's identity.
    pub const fn dump(&self) -> DumpId {
        self.dump
    }
    /// Borrow supplied exact release association facts.
    pub const fn release(&self) -> &ReleaseSymbols {
        &self.release
    }
    /// Return actual exception address in the faulting process address space.
    pub const fn address(&self) -> u64 {
        self.address
    }
    /// Return frame byte offset from the owning module's image base.
    pub const fn module_offset(&self) -> u64 {
        self.module_offset
    }
    /// Borrow the exact bounded resolved function spelling.
    pub const fn symbol(&self) -> &SymbolName {
        &self.symbol
    }
    /// Borrow the source-line fact; absence never fabricates a line.
    pub const fn source(&self) -> Option<&SourceLine> {
        self.source.as_ref()
    }
}
/// Categorical symbol-resolution failure; native causes remain in the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolFailure {
    /// Native dump is unavailable.
    MissingDump,
    /// Dump metadata/stream bounds are invalid.
    CorruptDump,
    /// No exact release symbol artifact is available.
    MissingSymbols,
    /// Build, architecture, GUID/age or digest did not match.
    WrongRelease,
    /// Exception address does not belong to a validated module.
    UnknownModule,
    /// Exact symbols did not resolve the actual exception address.
    UnresolvedAddress,
    /// Original helper deadline elapsed.
    DeadlineElapsed,
    /// Helper failed/panicked or produced no authenticated terminal result.
    HelperFailed,
}
/// Validated raw exception/module facts without any release-symbol association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawFaultFrame {
    dump: DumpId,
    module: FileLabel,
    address: u64,
    module_offset: u64,
}
impl RawFaultFrame {
    /// Retain externally validated dump/module/address facts; performs no native lookup.
    pub const fn new(dump: DumpId, module: FileLabel, address: u64, module_offset: u64) -> Self {
        Self { dump, module, address, module_offset }
    }
    /// Return the exact finished dump identity.
    pub const fn dump(&self) -> DumpId {
        self.dump
    }
    /// Borrow the validated module basename without granting filesystem access.
    pub const fn module(&self) -> &FileLabel {
        &self.module
    }
    /// Return the actual exception address in the faulting process address space.
    pub const fn address(&self) -> u64 {
        self.address
    }
    /// Return the validated byte offset from the owning module's image base.
    pub const fn module_offset(&self) -> u64 {
        self.module_offset
    }
}
/// Explicit resolution failure preserving raw facts only when their sources were valid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnresolvedSymbols {
    failure: SymbolFailure,
    raw_frame: Option<RawFaultFrame>,
}
impl UnresolvedSymbols {
    /// Retain an observed failure and optional independently validated raw frame.
    /// # Errors
    /// Rejects a raw frame when the dump or owning module was missing or invalid.
    pub fn new(
        failure: SymbolFailure,
        raw_frame: Option<RawFaultFrame>,
    ) -> Result<Self, IncidentError> {
        if raw_frame.is_some()
            && matches!(
                failure,
                SymbolFailure::MissingDump
                    | SymbolFailure::CorruptDump
                    | SymbolFailure::UnknownModule
            )
        {
            return Err(IncidentError::MismatchedFacts);
        }
        Ok(Self { failure, raw_frame })
    }
    /// Return the supplied categorical failure, without native errors or reclassification.
    pub const fn failure(&self) -> SymbolFailure {
        self.failure
    }
    /// Borrow available raw facts; absence never fabricates module or address information.
    pub const fn raw_frame(&self) -> Option<&RawFaultFrame> {
        self.raw_frame.as_ref()
    }
}
/// Exactly one failing-frame outcome; this type never asserts a fully unwound stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolOutcome {
    /// No symbol operation has supplied a result.
    NotRequested,
    /// Symbol resolution explicitly failed or remained unavailable.
    Unresolved(UnresolvedSymbols),
    /// Actual single failing-frame result, with optional source line.
    Resolved(FailingFrame),
}
