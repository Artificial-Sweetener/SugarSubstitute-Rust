// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact release association and one actual failing frame without symbol lookup authority.
use super::{IncidentStoreError, codec::required_option};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::incident::{
    BuildIdentity, DumpId, FailingFrame, FileLabel, PdbIdentity, RawFaultFrame, ReleaseSymbols,
    SourceLine, SymbolArchitecture, SymbolFailure, SymbolName, SymbolOutcome, UnresolvedSymbols,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseRecord {
    build: String,
    module: String,
    architecture: ArchitectureRecord,
    pdb_guid: [u8; 16],
    pdb_age: u32,
    image_sha256: [u8; 32],
    symbols_sha256: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArchitectureRecord {
    X86_64,
}
impl ReleaseRecord {
    pub(super) fn into_release(self) -> Result<ReleaseSymbols, IncidentStoreError> {
        Ok(ReleaseSymbols::new(
            BuildIdentity::new(&self.build)?,
            FileLabel::new(&self.module)?,
            match self.architecture {
                ArchitectureRecord::X86_64 => SymbolArchitecture::X86_64,
            },
            PdbIdentity::new(self.pdb_guid, self.pdb_age)?,
            self.image_sha256,
            self.symbols_sha256,
        ))
    }
}
impl From<&ReleaseSymbols> for ReleaseRecord {
    fn from(value: &ReleaseSymbols) -> Self {
        Self {
            build: value.build().as_str().into(),
            module: value.module().as_str().into(),
            architecture: match value.architecture() {
                SymbolArchitecture::X86_64 => ArchitectureRecord::X86_64,
            },
            pdb_guid: *value.pdb().guid(),
            pdb_age: value.pdb().age(),
            image_sha256: *value.image_sha256(),
            symbols_sha256: *value.symbols_sha256(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineRecord {
    file: String,
    line: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRecord {
    dump: [u8; 16],
    module: String,
    address: u64,
    module_offset: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FrameRecord {
    dump: [u8; 16],
    release: ReleaseRecord,
    address: u64,
    module_offset: u64,
    symbol: String,
    #[serde(deserialize_with = "required_option")]
    source: Option<LineRecord>,
}
impl FrameRecord {
    fn into_frame(self) -> Result<FailingFrame, IncidentStoreError> {
        Ok(FailingFrame::new(
            DumpId::new(self.dump)?,
            self.release.into_release()?,
            self.address,
            self.module_offset,
            SymbolName::new(&self.symbol)?,
            self.source
                .map(|source| SourceLine::new(FileLabel::new(&source.file)?, source.line))
                .transpose()?,
        ))
    }
}
impl From<&FailingFrame> for FrameRecord {
    fn from(value: &FailingFrame) -> Self {
        Self {
            dump: *value.dump().bytes(),
            release: value.release().into(),
            address: value.address(),
            module_offset: value.module_offset(),
            symbol: value.symbol().as_str().into(),
            source: value.source().map(|source| LineRecord {
                file: source.file().as_str().into(),
                line: source.line(),
            }),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum FailureRecord {
    MissingDump,
    CorruptDump,
    MissingSymbols,
    WrongRelease,
    UnknownModule,
    UnresolvedAddress,
    DeadlineElapsed,
    HelperFailed,
}
impl FailureRecord {
    fn into_failure(self) -> SymbolFailure {
        match self {
            Self::MissingDump => SymbolFailure::MissingDump,
            Self::CorruptDump => SymbolFailure::CorruptDump,
            Self::MissingSymbols => SymbolFailure::MissingSymbols,
            Self::WrongRelease => SymbolFailure::WrongRelease,
            Self::UnknownModule => SymbolFailure::UnknownModule,
            Self::UnresolvedAddress => SymbolFailure::UnresolvedAddress,
            Self::DeadlineElapsed => SymbolFailure::DeadlineElapsed,
            Self::HelperFailed => SymbolFailure::HelperFailed,
        }
    }
}
impl From<SymbolFailure> for FailureRecord {
    fn from(value: SymbolFailure) -> Self {
        match value {
            SymbolFailure::MissingDump => Self::MissingDump,
            SymbolFailure::CorruptDump => Self::CorruptDump,
            SymbolFailure::MissingSymbols => Self::MissingSymbols,
            SymbolFailure::WrongRelease => Self::WrongRelease,
            SymbolFailure::UnknownModule => Self::UnknownModule,
            SymbolFailure::UnresolvedAddress => Self::UnresolvedAddress,
            SymbolFailure::DeadlineElapsed => Self::DeadlineElapsed,
            SymbolFailure::HelperFailed => Self::HelperFailed,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum SymbolsRecord {
    NotRequested {},
    Unresolved {
        failure: FailureRecord,
        #[serde(deserialize_with = "required_option")]
        raw_frame: Option<RawRecord>,
    },
    Resolved {
        frame: FrameRecord,
    },
}
impl SymbolsRecord {
    pub(super) fn into_facts(self) -> Result<SymbolOutcome, IncidentStoreError> {
        Ok(match self {
            Self::NotRequested {} => SymbolOutcome::NotRequested,
            Self::Resolved { frame } => SymbolOutcome::Resolved(frame.into_frame()?),
            Self::Unresolved { failure, raw_frame } => {
                SymbolOutcome::Unresolved(UnresolvedSymbols::new(
                    failure.into_failure(),
                    raw_frame
                        .map(|raw| {
                            Ok::<_, IncidentStoreError>(RawFaultFrame::new(
                                DumpId::new(raw.dump)?,
                                FileLabel::new(&raw.module)?,
                                raw.address,
                                raw.module_offset,
                            ))
                        })
                        .transpose()?,
                )?)
            }
        })
    }
}
impl From<&SymbolOutcome> for SymbolsRecord {
    fn from(value: &SymbolOutcome) -> Self {
        match value {
            SymbolOutcome::NotRequested => Self::NotRequested {},
            SymbolOutcome::Resolved(frame) => Self::Resolved { frame: frame.into() },
            SymbolOutcome::Unresolved(value) => Self::Unresolved {
                failure: value.failure().into(),
                raw_frame: value.raw_frame().map(|raw| RawRecord {
                    dump: *raw.dump().bytes(),
                    module: raw.module().as_str().into(),
                    address: raw.address(),
                    module_offset: raw.module_offset(),
                }),
            },
        }
    }
}
