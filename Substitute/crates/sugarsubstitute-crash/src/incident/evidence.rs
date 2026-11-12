// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Format an explicit allowlist of owned facts without arbitrary errors, arguments or environment.
use super::{HostFailureFact, IncidentStoreError};
use sugarsubstitute_contracts::incident::{
    CaptureEvidence, ExitAttribution, FailureEvidence, IncidentError, IncidentStage,
};

const CHUNK_BYTES: usize = 65_536;
const FACT_BOUND: usize = 2048;
const OMITTED: &[u8] = b"[additional diagnostic facts omitted]\n";

/// Closed diagnostic observation categories; these carry no text or native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentLogCode {
    /// A finite owned operation entered admission.
    OperationAdmitted,
    /// A finite owned operation reached its observed result.
    OperationObserved,
    /// A result was rejected because its identity was stale or foreign.
    StaleResultRejected,
    /// Owner retirement began, without claiming it completed.
    RetirementStarted,
    /// Actual owner retirement was observed.
    RetirementObserved,
}
/// Explicit allowlist for bounded redacted text; arbitrary strings cannot enter this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedactedDiagnosticFact {
    /// Last observed lifecycle stage.
    Stage(IncidentStage),
    /// Independently supplied fault category or actual code/address.
    Failure(FailureEvidence),
    /// Capture availability, without dump contents or paths.
    Capture(CaptureEvidence),
    /// Authoritative supplied exit classification and actual status.
    Exit(ExitAttribution),
    /// Closed structured operation observation and positive or zero numeric sequence.
    Log {
        /// Stable category rather than native error text.
        code: IncidentLogCode,
        /// Exact caller observation sequence; this grants no ordering authority.
        sequence: u64,
    },
}
/// UTF-8 allowlist output within one 64-KiB storage chunk, including omission marker.
/// The private constructor prevents raw native errors, argv or environment strings entering it.
pub struct RedactedTextChunk {
    bytes: Vec<u8>,
    original_bytes: u64,
    omitted_bytes: u64,
    host_failure: bool,
}
impl RedactedTextChunk {
    /// Format one validated host failure without reserving a complete general-purpose tail.
    pub fn from_host_failure(fact: &HostFailureFact) -> Self {
        let bytes = fact.to_string().into_bytes();
        Self { original_bytes: bytes.len() as u64, bytes, omitted_bytes: 0, host_failure: true }
    }
    /// Format 1..=2048 typed facts while retaining explicit generated-source omission counts.
    /// # Errors
    /// Rejects empty or oversized fact inventories. No clock, native path or error is sampled.
    pub fn from_facts(facts: &[RedactedDiagnosticFact]) -> Result<Self, IncidentStoreError> {
        if facts.is_empty() || facts.len() > FACT_BOUND {
            return Err(IncidentError::InvalidEvidence.into());
        }
        let mut bytes = Vec::with_capacity(CHUNK_BYTES);
        let mut original_bytes = 0_u64;
        let mut retained_source = 0_u64;
        let mut truncated = false;
        for fact in facts {
            let line = format_fact(*fact);
            original_bytes = original_bytes
                .checked_add(line.len() as u64)
                .ok_or(IncidentError::InvalidEvidence)?;
            if !truncated && bytes.len() + line.len() <= CHUNK_BYTES - OMITTED.len() {
                bytes.extend_from_slice(line.as_bytes());
                retained_source += line.len() as u64;
            } else {
                truncated = true;
            }
        }
        if truncated {
            bytes.extend_from_slice(OMITTED);
        }
        Ok(Self {
            bytes,
            original_bytes,
            omitted_bytes: original_bytes - retained_source,
            host_failure: false,
        })
    }
    /// Borrow actual UTF-8 bytes, including any explicit omission marker.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Whether this appendix was generated from a validated recorded host failure.
    pub const fn contains_host_failure(&self) -> bool {
        self.host_failure
    }
    /// Return total generated allowlist source bytes before bounded omission.
    pub const fn original_bytes(&self) -> u64 {
        self.original_bytes
    }
    /// Return omitted source bytes; marker bytes are counted only in retained output.
    pub const fn omitted_bytes(&self) -> u64 {
        self.omitted_bytes
    }
}
fn format_fact(fact: RedactedDiagnosticFact) -> String {
    match fact {
        RedactedDiagnosticFact::Stage(stage) => format!("stage: {stage:?}\n"),
        RedactedDiagnosticFact::Failure(FailureEvidence::NativeFault { code, address }) => {
            format!("fault: native code={code:#010x} address={address:#018x}\n")
        }
        RedactedDiagnosticFact::Failure(failure) => format!("fault: {failure:?}\n"),
        RedactedDiagnosticFact::Capture(CaptureEvidence::Finished(_)) => {
            "capture: finished\n".into()
        }
        RedactedDiagnosticFact::Capture(capture) => format!("capture: {capture:?}\n"),
        RedactedDiagnosticFact::Exit(exit) => format!("exit: {exit:?}\n"),
        RedactedDiagnosticFact::Log { code, sequence } => {
            format!("observation: {code:?} sequence={sequence}\n")
        }
    }
}
