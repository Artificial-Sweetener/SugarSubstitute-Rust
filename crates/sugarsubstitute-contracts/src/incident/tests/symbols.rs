// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve independently validated raw facts without inventing release associations.
use super::*;

fn raw() -> Result<RawFaultFrame, IncidentError> {
    Ok(RawFaultFrame::new(
        DumpId::new([7; 16])?,
        FileLabel::new("actual-module.dll")?,
        0x7010,
        0x10,
    ))
}

#[test]
fn unresolved_failures_preserve_raw_frame_only_with_valid_source_facts()
-> Result<(), Box<dyn std::error::Error>> {
    for failure in [
        SymbolFailure::MissingSymbols,
        SymbolFailure::WrongRelease,
        SymbolFailure::UnresolvedAddress,
        SymbolFailure::DeadlineElapsed,
        SymbolFailure::HelperFailed,
    ] {
        let supplied = raw()?;
        let unresolved = UnresolvedSymbols::new(failure, Some(supplied.clone()))?;
        assert_eq!(unresolved.failure(), failure);
        assert_eq!(unresolved.raw_frame(), Some(&supplied));
        let retained = unresolved.raw_frame().ok_or("raw frame disappeared")?;
        assert_eq!(retained.dump().bytes(), &[7; 16]);
        assert_eq!(retained.module().as_str(), "actual-module.dll");
        assert_eq!(retained.address(), 0x7010);
        assert_eq!(retained.module_offset(), 0x10);
        assert_eq!(UnresolvedSymbols::new(failure, None)?.raw_frame(), None);
    }
    for failure in
        [SymbolFailure::MissingDump, SymbolFailure::CorruptDump, SymbolFailure::UnknownModule]
    {
        assert_eq!(
            UnresolvedSymbols::new(failure, Some(raw()?)),
            Err(IncidentError::MismatchedFacts)
        );
        assert_eq!(UnresolvedSymbols::new(failure, None)?.failure(), failure);
    }
    Ok(())
}

#[test]
fn unresolved_raw_frame_preserves_attribution_and_requires_exact_finished_dump_and_address()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = raw()?;
    let key = desktop()?;
    for failure in [SymbolFailure::MissingSymbols, SymbolFailure::WrongRelease] {
        let symbols =
            SymbolOutcome::Unresolved(UnresolvedSymbols::new(failure, Some(raw.clone()))?);
        for evidence in [
            FailureEvidence::Unexplained,
            FailureEvidence::Abort,
            FailureEvidence::RustPanic,
            FailureEvidence::NativeFault { code: 0xc000_0005, address: raw.address() },
        ] {
            let summary = IncidentSummary::new(
                seed(key)?,
                IncidentRevision::new(1)?,
                facts(
                    ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
                    evidence,
                    CaptureEvidence::Finished(raw.dump()),
                    symbols.clone(),
                )?,
            )?;
            assert_eq!(summary.facts().failure(), evidence);
            assert_eq!(summary.facts().symbols(), &symbols);
            assert_eq!(summary.seed().build().as_str(), "release-1");
        }
        let mut other_uuid = *raw.dump().bytes();
        other_uuid[15] ^= 1;
        for (capture, evidence) in [
            (CaptureEvidence::Finished(DumpId::new(other_uuid)?), FailureEvidence::Unexplained),
            (CaptureEvidence::Awaiting, FailureEvidence::Unexplained),
            (CaptureEvidence::Unavailable(CaptureAbsence::Missing), FailureEvidence::Unexplained),
            (
                CaptureEvidence::Finished(raw.dump()),
                FailureEvidence::NativeFault { code: 0xc000_0005, address: raw.address() + 1 },
            ),
        ] {
            assert_eq!(
                IncidentSummary::new(
                    seed(key)?,
                    IncidentRevision::new(1)?,
                    facts(
                        ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
                        evidence,
                        capture,
                        symbols.clone()
                    )?
                ),
                Err(IncidentError::MismatchedFacts)
            );
        }
    }
    Ok(())
}
