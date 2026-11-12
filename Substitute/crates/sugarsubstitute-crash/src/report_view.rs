// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Format bounded redacted incident facts without granting storage or native authority.
#[cfg(test)]
mod tests;
mod writer;
use crate::incident::RedactedTextChunk;
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryClaimError, MemoryLease},
    incident::*,
    localization::{incident::ReportMessages, messages},
};
use writer::Writer;

/// Owned immutable redacted UTF8 with its actual allocation charge.
/// Dropping this data performs no acknowledgement, export or cleanup.
pub struct ReportView {
    text: String,
    binding: ReportBinding,
    claim: MemoryClaim,
}
impl ReportView {
    /// Bound every formatted byte, including the localized omission marker and final LF.
    pub const MAX_BYTES: usize = MAX_TEXT_ATTACHMENT_BYTES as usize;
    /// Reserve header credit for charged transfer into native presentation.
    pub const TRANSFER_HEADER_BYTES: u64 = 512;

    /// Format supplied committed facts and at most four already redacted diagnostic tails.
    /// The coverage flag adds localized presentation copy within the same allocation bound.
    /// It neither reads files nor authenticates facts or grants native capture authority.
    ///
    /// # Errors
    /// Rejects mismatched binding/namespace, over-bound tail inventories, allocation
    /// failure or insufficient already admitted credit before retaining new text.
    pub fn format(
        summary: &IncidentSummary,
        binding: ReportBinding,
        evictions: SummaryEvictions,
        tails: &[RedactedTextChunk],
        locale: &str,
        lease: &MemoryLease,
        root_capture_degraded: bool,
    ) -> Result<Self, ReportViewError> {
        if summary.seed().key() != binding.incident()
            || summary.revision() != binding.revision()
            || evictions.namespace() != binding.incident().subject().coordinator().namespace
        {
            return Err(ReportViewError::Identity);
        }
        if tails.len() > 4 {
            return Err(ReportViewError::Tails);
        }
        let claim = lease
            .claim(Self::MAX_BYTES as u64 + Self::TRANSFER_HEADER_BYTES)
            .map_err(ReportViewError::Credit)?;
        let labels = messages(locale).report;
        let mut writer = Writer::new(Self::MAX_BYTES, labels.omitted)?;
        write_facts(
            &mut writer,
            summary,
            evictions,
            labels,
            tails.iter().any(RedactedTextChunk::contains_host_failure),
        );
        if root_capture_degraded {
            writer.line(labels.recovery_capture_degraded);
        }
        writer.line(labels.diagnostics);
        for tail in tails {
            writer.bytes(tail.as_bytes());
        }
        Ok(Self { text: writer.finish(), binding, claim })
    }
    /// Borrow the complete redacted UTF8 text, with LF line endings and final LF.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Return exact incident/revision/session matching facts, not native authority.
    pub const fn binding(&self) -> ReportBinding {
        self.binding
    }
    /// Transfer the same allocation and noncloneable claim into another inert view owner.
    pub fn into_parts(self) -> (ReportBinding, String, MemoryClaim) {
        (self.binding, self.text, self.claim)
    }
}

fn write_facts(
    w: &mut Writer,
    summary: &IncidentSummary,
    evictions: SummaryEvictions,
    l: &ReportMessages,
    host_failure: bool,
) {
    w.line(l.title);
    w.line(l.explanation);
    w.named(l.build, format_args!("{}", summary.seed().build().as_str()));
    w.named(l.revision, format_args!("{}", summary.revision().get()));
    match summary.facts().exit() {
        ExitAttribution::Desktop { status, .. } | ExitAttribution::Supervisor { status } => {
            w.named(l.exit_code, format_args!("{status}"));
        }
        ExitAttribution::CaptureOnly => w.line(l.exit_unobserved),
    }
    match summary.facts().failure() {
        FailureEvidence::Unexplained => {
            w.line(if host_failure { l.host_failed } else { l.unexplained })
        }
        FailureEvidence::Startup(code) => {
            w.named(l.startup_failed, format_args!("{}", l.startup_failure[startup_reason(code)]))
        }
        FailureEvidence::RustPanic => w.line(l.panic),
        FailureEvidence::Abort => w.line(l.abort),
        FailureEvidence::NativeFault { code, address } => {
            w.named(l.native_fault, format_args!("0x{code:08x} @ 0x{address:x}"));
        }
    }
    w.named(
        l.capture,
        format_args!(
            "{}",
            match summary.facts().capture() {
                CaptureEvidence::Awaiting => l.pending,
                CaptureEvidence::Unavailable(reason) => l.capture_absence[capture_reason(reason)],
                CaptureEvidence::Finished(_) => l.retained,
            }
        ),
    );
    w.named(
        l.retained,
        format_args!(
            "{}",
            match summary.facts().payload() {
                PayloadDisposition::Retained(_) => l.retained,
                PayloadDisposition::Discarded(_) => l.discarded,
                PayloadDisposition::Unavailable => l.unavailable,
                PayloadDisposition::Quarantined => l.quarantined,
            }
        ),
    );
    match summary.facts().symbols() {
        SymbolOutcome::Resolved(frame) => {
            w.named(
                l.symbols,
                format_args!(
                    "{} +0x{:x}: {}",
                    frame.release().module().as_str(),
                    frame.module_offset(),
                    frame.symbol().as_str()
                ),
            );
            if let Some(source) = frame.source() {
                w.line_args(format_args!("{}:{}", source.file().as_str(), source.line()));
            }
        }
        SymbolOutcome::Unresolved(outcome) => {
            w.named(
                l.symbols,
                format_args!("{}", l.symbol_failure[symbol_reason(outcome.failure())]),
            );
            if let Some(frame) = outcome.raw_frame() {
                w.line_args(format_args!(
                    "{} +0x{:x} @ 0x{:x}",
                    frame.module().as_str(),
                    frame.module_offset(),
                    frame.address()
                ));
            }
        }
        SymbolOutcome::NotRequested => w.named(
            l.symbols,
            format_args!(
                "{}",
                if host_failure
                    && matches!(summary.facts().capture(), CaptureEvidence::Unavailable(_))
                {
                    l.unavailable
                } else {
                    l.pending
                }
            ),
        ),
    }
    w.named(
        l.evictions,
        format_args!(
            "{}/{}/{}",
            evictions.pending(),
            evictions.acknowledged(),
            evictions.quarantined()
        ),
    );
}
fn capture_reason(reason: CaptureAbsence) -> usize {
    match reason {
        CaptureAbsence::HandlerUnavailable => 0,
        CaptureAbsence::HandlerLost => 1,
        CaptureAbsence::Missing => 2,
        CaptureAbsence::Busy => 3,
        CaptureAbsence::DeadlineElapsed => 4,
        CaptureAbsence::WrongIdentity => 5,
        CaptureAbsence::Corrupt => 6,
        CaptureAbsence::Oversized => 7,
    }
}
fn startup_reason(reason: sugarsubstitute_contracts::startup::StartupFailureCode) -> usize {
    use sugarsubstitute_contracts::startup::StartupFailureCode;
    match reason {
        StartupFailureCode::ConnectionPreparation => 0,
        StartupFailureCode::ConnectionUnavailable => 1,
        StartupFailureCode::ProcessExited => 2,
        StartupFailureCode::TimedOut => 3,
        StartupFailureCode::Presentation => 4,
    }
}
fn symbol_reason(reason: SymbolFailure) -> usize {
    match reason {
        SymbolFailure::MissingDump => 0,
        SymbolFailure::CorruptDump => 1,
        SymbolFailure::MissingSymbols => 2,
        SymbolFailure::WrongRelease => 3,
        SymbolFailure::UnknownModule => 4,
        SymbolFailure::UnresolvedAddress => 5,
        SymbolFailure::DeadlineElapsed => 6,
        SymbolFailure::HelperFailed => 7,
    }
}

/// Reject view construction without formatting private paths or native causes.
#[derive(Debug)]
pub enum ReportViewError {
    /// Allocation or omission framing exceeded the bounded writer contract.
    Bounds,
    /// Full binding or ledger namespace disagrees with supplied facts.
    Identity,
    /// More than four redacted diagnostic tails were supplied.
    Tails,
    /// Existing reservation cannot cover this owned allocation.
    Credit(MemoryClaimError),
    /// Checked bounded allocation failed.
    Allocation(TryReserveError),
}
impl fmt::Display for ReportViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Incident view rejected: {self:?}")
    }
}
impl Error for ReportViewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Credit(e) => Some(e),
            Self::Allocation(e) => Some(e),
            _ => None,
        }
    }
}
