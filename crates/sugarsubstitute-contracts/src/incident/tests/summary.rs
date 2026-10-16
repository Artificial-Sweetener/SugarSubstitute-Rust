// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject contradictory summary identities while preserving upstream owner decisions.
use super::*;

#[test]
fn seed_rejects_cross_subject_initialization_and_retains_bounded_context()
-> Result<(), Box<dyn std::error::Error>> {
    let key = desktop()?;
    assert_eq!(
        IncidentSeed::new(
            key,
            IncidentRevision::new(1)?,
            BuildIdentity::new("release")?,
            IncidentUtc::from_unix_seconds(100)?,
            IncidentStage::SupervisorInitialization,
        ),
        Err(IncidentError::MismatchedFacts)
    );
    assert_eq!(
        IncidentSeed::new(
            IncidentKey::new(IncidentSubject::Supervisor(owner()?)),
            IncidentRevision::new(1)?,
            BuildIdentity::new("release")?,
            IncidentUtc::from_unix_seconds(100)?,
            IncidentStage::DesktopInitialization,
        ),
        Err(IncidentError::MismatchedFacts)
    );
    let seed = seed(key)?;
    assert_eq!(seed.key(), key);
    assert_eq!(seed.revision().get(), 1);
    assert_eq!(seed.build().as_str(), "release-1");
    assert_eq!(seed.created().unix_seconds(), 100);
    assert_eq!(seed.stage(), IncidentStage::Startup);
    Ok(())
}

#[test]
fn summary_preserves_all_m1_kinds_and_statuses_without_reclassification()
-> Result<(), Box<dyn std::error::Error>> {
    let key = desktop()?;
    for kind in [
        DesktopExitKind::Normal,
        DesktopExitKind::StartupCancelled,
        DesktopExitKind::StartupFailed,
        DesktopExitKind::Unexpected,
    ] {
        for status in [0, 37, 259, -9] {
            let supplied = ExitAttribution::Desktop { kind, status };
            let summary = IncidentSummary::new(
                seed(key)?,
                IncidentRevision::new(1)?,
                facts(
                    supplied,
                    FailureEvidence::Unexplained,
                    CaptureEvidence::Unavailable(CaptureAbsence::Missing),
                    SymbolOutcome::NotRequested,
                )?,
            )?;
            assert_eq!(summary.facts().exit(), supplied);
            assert_eq!(summary.facts().failure(), FailureEvidence::Unexplained);
            assert_eq!(summary.seed().key(), key);
            assert_eq!(summary.revision().get(), 1);
        }
    }
    Ok(())
}

#[test]
fn summary_rejects_subject_revision_and_time_mismatches() -> Result<(), Box<dyn std::error::Error>>
{
    let key = desktop()?;
    let supplied = facts(
        ExitAttribution::Supervisor { status: 37 },
        FailureEvidence::Unexplained,
        CaptureEvidence::Awaiting,
        SymbolOutcome::NotRequested,
    )?;
    assert_eq!(
        IncidentSummary::new(seed(key)?, IncidentRevision::new(1)?, supplied.clone()),
        Err(IncidentError::MismatchedFacts)
    );
    assert!(
        IncidentSummary::new(
            seed(IncidentKey::new(IncidentSubject::Supervisor(owner()?)))?,
            IncidentRevision::new(1)?,
            supplied,
        )
        .is_ok()
    );
    for (seed_revision, created) in [(2, 100), (1, 201)] {
        let stale = IncidentSeed::new(
            key,
            IncidentRevision::new(seed_revision)?,
            BuildIdentity::new("release-1")?,
            IncidentUtc::from_unix_seconds(created)?,
            IncidentStage::Startup,
        )?;
        assert_eq!(
            IncidentSummary::new(
                stale,
                IncidentRevision::new(1)?,
                facts(
                    ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
                    FailureEvidence::Unexplained,
                    CaptureEvidence::Awaiting,
                    SymbolOutcome::NotRequested,
                )?,
            ),
            Err(IncidentError::MismatchedFacts)
        );
    }
    Ok(())
}

#[test]
fn resolved_frame_requires_exact_release_dump_and_actual_fault_address()
-> Result<(), Box<dyn std::error::Error>> {
    let key = desktop()?;
    let dump = DumpId::new([1; 16])?;
    for (build, capture, address) in [
        ("foreign-release", CaptureEvidence::Finished(dump), 0x1010),
        ("release-1", CaptureEvidence::Finished(DumpId::new([2; 16])?), 0x1010),
        ("release-1", CaptureEvidence::Unavailable(CaptureAbsence::Missing), 0x1010),
        ("release-1", CaptureEvidence::Finished(dump), 0x1011),
    ] {
        assert_eq!(
            IncidentSummary::new(
                seed(key)?,
                IncidentRevision::new(1)?,
                facts(
                    ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 37 },
                    FailureEvidence::NativeFault { code: 0xc000_0005, address },
                    capture,
                    SymbolOutcome::Resolved(frame(build, dump)?),
                )?,
            ),
            Err(IncidentError::MismatchedFacts)
        );
    }
    let summary = IncidentSummary::new(
        seed(key)?,
        IncidentRevision::new(2)?,
        facts(
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 37 },
            FailureEvidence::NativeFault { code: 0xc000_0005, address: 0x1010 },
            CaptureEvidence::Finished(dump),
            SymbolOutcome::Resolved(frame("release-1", dump)?),
        )?,
    )?;
    let SymbolOutcome::Resolved(frame) = summary.facts().symbols() else {
        return Err("actual frame disappeared".into());
    };
    assert_eq!(frame.dump(), dump);
    assert_eq!(frame.address(), 0x1010);
    assert_eq!(frame.module_offset(), 0x10);
    assert_eq!(frame.symbol().as_str(), "sugarsubstitute::fault_fixture");
    assert_eq!(frame.release().module().as_str(), "sugarsubstitute.exe");
    assert_eq!(frame.release().architecture(), SymbolArchitecture::X86_64);
    assert_eq!(frame.release().pdb().age(), 1);
    assert_eq!(frame.release().image_sha256(), &[4; 32]);
    assert_eq!(frame.release().symbols_sha256(), &[5; 32]);
    let source = frame.source().ok_or("actual source line disappeared")?;
    assert_eq!(source.file().as_str(), "fixture.rs");
    assert_eq!(source.line(), 17);
    Ok(())
}

#[test]
fn actual_frame_without_source_line_and_unresolved_symbols_remain_honest()
-> Result<(), Box<dyn std::error::Error>> {
    let resolved = frame("release-1", DumpId::new([1; 16])?)?;
    let without_line = FailingFrame::new(
        resolved.dump(),
        resolved.release().clone(),
        resolved.address(),
        resolved.module_offset(),
        resolved.symbol().clone(),
        None,
    );
    assert_eq!(without_line.source(), None);
    let key = desktop()?;
    let summary = IncidentSummary::new(
        seed(key)?,
        IncidentRevision::new(1)?,
        facts(
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Finished(resolved.dump()),
            SymbolOutcome::Unresolved(UnresolvedSymbols::new(SymbolFailure::WrongRelease, None)?),
        )?,
    )?;
    assert_eq!(
        summary.facts().symbols(),
        &SymbolOutcome::Unresolved(UnresolvedSymbols::new(SymbolFailure::WrongRelease, None)?)
    );
    assert_eq!(summary.facts().failure(), FailureEvidence::Unexplained);
    Ok(())
}

#[test]
fn capture_only_summary_keeps_subject_without_inventing_a_process_status()
-> Result<(), Box<dyn std::error::Error>> {
    for key in [desktop()?, IncidentKey::new(IncidentSubject::Supervisor(owner()?))] {
        let supplied = FailureEvidence::NativeFault { code: 0xc0000005, address: 0x7010 };
        let summary = IncidentSummary::new(
            seed(key)?,
            IncidentRevision::new(1)?,
            facts(
                ExitAttribution::CaptureOnly,
                supplied,
                CaptureEvidence::Finished(DumpId::new([5; 16])?),
                SymbolOutcome::NotRequested,
            )?,
        )?;
        assert_eq!(summary.seed().key(), key);
        assert_eq!(summary.facts().exit(), ExitAttribution::CaptureOnly);
        assert_eq!(summary.facts().failure(), supplied);
    }
    Ok(())
}
#[test]
fn capture_only_summary_requires_both_finished_capture_and_actual_native_exception_facts()
-> Result<(), Box<dyn std::error::Error>> {
    for (capture, failure) in [
        (
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            FailureEvidence::NativeFault { code: 1, address: 2 },
        ),
        (CaptureEvidence::Awaiting, FailureEvidence::NativeFault { code: 1, address: 2 }),
        (CaptureEvidence::Finished(DumpId::new([5; 16])?), FailureEvidence::Unexplained),
        (CaptureEvidence::Finished(DumpId::new([5; 16])?), FailureEvidence::RustPanic),
    ] {
        assert!(
            IncidentSummary::new(
                seed(desktop()?)?,
                IncidentRevision::new(1)?,
                facts(ExitAttribution::CaptureOnly, failure, capture, SymbolOutcome::NotRequested)?
            )
            .is_err()
        );
    }
    Ok(())
}
