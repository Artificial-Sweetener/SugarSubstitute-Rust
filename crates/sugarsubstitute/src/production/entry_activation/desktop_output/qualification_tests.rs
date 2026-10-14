// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual compact producers, closed stream decoding and passive Root relay.
use super::super::diagnostic::{self, Diagnostic, Observation};
use super::*;
use sugarsubstitute_contracts::{
    execution::Generation, lifecycle::*, startup::LaunchId, transcript::OutputStream,
};
use sugarsubstitute_desktop::qualification_trace::{self as producer, Cause, PngProof};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn formatted(emit: impl FnOnce()) -> Result<Vec<u8>> {
    super::super::diagnostic::tests::formatted_owner_diagnostics(emit)
}
fn main() -> Result<MainIdentity> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        },
        launch: LaunchId::new(u64::MAX)?,
        navigation: Generation::new(u64::MAX)?,
    })
}
fn proof() -> PngProof {
    PngProof {
        bytes: 2 * 1024 * 1024,
        digest: [255; 32],
        native_navigation: u64::MAX,
        width: u32::MAX,
        height: u32::MAX,
        dpi: u32::MAX,
    }
}
fn parsed(bytes: &[u8]) -> Vec<Qualification> {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stderr, 1, false, false, bytes, |record| {
        if let Observation::Qualification(record) = record {
            seen.push(record);
        }
    });
    seen
}
#[test]
fn actual_compact_png_formatter_decoder_and_root_relay_preserve_original_facts() -> Result {
    let main = main()?;
    let proof = proof();
    let bytes = formatted(|| producer::persisted(main, proof))?;
    assert!(bytes.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let at = bytes.len() / 2;
    owner.feed(OutputStream::Stderr, 1, false, false, &bytes[..at], |value| seen.push(value));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 2, false, false, &bytes[at..], |value| seen.push(value));
    let [Observation::Qualification(record)] = seen.as_slice() else {
        return Err("Actual PNG record not decoded".into());
    };
    assert_eq!(record.detail, Detail::Persisted(proof));
    let output = formatted(|| {
        assert!(relay(main.run, Some(main), *record));
    })?;
    let output = String::from_utf8(output)?;
    assert!(output.contains("operation=\"main_preview_persisted\""));
    assert!(output.contains(&format!("identity={main:?} png_bytes={}", proof.bytes)));
    assert!(output.contains(&format!(
        "png_sha256={} native_navigation={}",
        "f".repeat(64),
        u64::MAX
    )));
    assert!(output.contains(&format!(
        "viewport_width={} viewport_height={} dpi={}",
        u32::MAX,
        u32::MAX,
        u32::MAX
    )));
    Ok(())
}
#[test]
fn every_actual_stage_and_first_cause_has_closed_end_to_end_relay() -> Result {
    let main = main()?;
    let stages = [
        Progress::EnvironmentDispatched,
        Progress::BrowserDispatched,
        Progress::BrowserAuthorized,
        Progress::NavigationStarted,
        Progress::NavigationCompleted,
        Progress::FrontendReady,
        Progress::FrontendFailed,
        Progress::RevealDispatched,
        Progress::MainRevealed,
        Progress::PreviewRequested,
        Progress::PreviewDispatched,
    ];
    let bytes = formatted(|| {
        for stage in stages {
            producer::progress(stage, main);
        }
        producer::failure(Some(main), Cause::Deadline);
        producer::failure(None, Cause::Arming);
        producer::finished();
    })?;
    assert!(bytes.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
    let records = parsed(&bytes);
    assert_eq!(records.len(), 14);
    for (record, stage) in records.iter().zip(stages) {
        assert_eq!(record.detail, Detail::Progress(stage));
    }
    assert_eq!(records[11].detail, Detail::Failed(Cause::Deadline));
    assert_eq!(records[12].detail, Detail::Failed(Cause::Arming));
    assert_eq!(records[13].detail, Detail::Finished);
    let relay_output = formatted(|| {
        for record in records {
            assert!(relay(main.run, Some(main), record));
        }
    })?;
    let relay_output = String::from_utf8(relay_output)?;
    assert!(relay_output.contains("operation=\"main_frontend_receipt\""));
    assert!(relay_output.contains("cause_kind=Deadline"));
    assert!(relay_output.contains("operation=\"production_desktop_retired\""));
    Ok(())
}
#[test]
fn authoritative_current_identity_rejects_absent_stale_and_foreign_main_without_relay() -> Result {
    let main = main()?;
    let record = parsed(&formatted(|| producer::persisted(main, proof()))?)[0];
    let stale = MainIdentity { launch: LaunchId::new(1)?, ..main };
    let foreign =
        MainIdentity { run: RunKey { instance: InstanceId::new(99)?, ..main.run }, ..main };
    let output = formatted(|| {
        for current in [None, Some(stale), Some(foreign)] {
            assert!(!relay(main.run, current, record));
        }
        assert!(!relay(foreign.run, Some(main), record));
    })?;
    assert!(output.is_empty());
    // Explicit absent-Main failure and genuine shutdown return are passive run-bound facts.
    for record in parsed(&formatted(|| {
        producer::failure(None, Cause::Deadline);
        producer::finished();
    })?) {
        assert!(relay(main.run, None, record));
    }
    Ok(())
}
#[test]
fn malformed_extra_duplicate_unknown_overflow_and_wrong_target_records_are_opaque() -> Result {
    let main = main()?;
    let bytes = formatted(|| producer::persisted(main, proof()))?;
    let valid = String::from_utf8(bytes)?;
    let malformed = [
        valid.replace("schema=1", "schema=2"),
        valid.replace("png_bytes=2097152", "png_bytes=2097153"),
        valid.replace("viewport_width=4294967295", "viewport_width=4294967296"),
        valid.replace("main=true", "main=false"),
        valid.replace("stage=\"PreviewPersisted\"", "stage=\"Unknown\""),
        valid.replace("png_sha256=ff", "png_sha256=gg"),
        valid.replace("dpi=4294967295", "dpi=0"),
        valid.replace("schema=1", "schema=1 schema=1"),
        valid.replace("schema=1", "schema=1 password=hidden"),
        valid.replace(
            "sugarsubstitute_desktop::qualification_trace:",
            "other::qualification_trace:",
        ),
        valid.replace("launch=18446744073709551615", "launch=18446744073709551616"),
        valid.replace("schema=1", "schema=+1"),
        format!("{} raw=hidden\n", valid.trim_end()),
    ];
    for bytes in malformed {
        assert!(parsed(bytes.as_bytes()).is_empty());
    }
    assert!(parsed(&[255, b'\n']).is_empty());
    assert!(parsed(format!("{}{}", "x".repeat(513), valid).as_bytes()).is_empty());
    assert!(parsed(b"private raw child stderr\n").is_empty());
    Ok(())
}
#[test]
fn actual_stream_owner_rejects_stdout_gaps_replays_and_partial_eof() -> Result {
    let main = main()?;
    let bytes = formatted(|| producer::progress(Progress::MainRevealed, main))?;
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let at = bytes.len() / 2;
    owner.feed(OutputStream::Stdout, 1, false, false, &bytes, |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 1, false, false, &bytes[..at], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 2, true, false, &bytes[at..], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 2, false, false, &bytes, |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 3, false, true, &bytes[..at], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 4, false, false, &bytes[at..], |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 5, false, false, &bytes, |v| seen.push(v));
    assert_eq!(seen.len(), 1);
    Ok(())
}
#[test]
fn qualification_shares_original_thirty_slots_and_keeps_two_real_failure_slots() -> Result {
    let main = main()?;
    let bytes = formatted(|| producer::progress(Progress::MainRevealed, main))?;
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    for sequence in 1..=40 {
        owner.feed(OutputStream::Stderr, sequence, false, false, &bytes, |v| seen.push(v));
    }
    assert_eq!(seen.len(), 30);
    owner.feed(
        OutputStream::Stderr,
        41,
        false,
        false,
        b"operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other\n",
        |v| seen.push(v),
    );
    owner.feed(OutputStream::Stderr,42,false,false,b"operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Frame code=JsonData\n",|v|seen.push(v));
    assert_eq!(seen.len(), 32);
    assert!(matches!(seen[30], Observation::Failure(_)));
    assert!(matches!(seen[31], Observation::MainFailure(_)));
    Ok(())
}
#[test]
fn compiled_diagnostic_variant_fits_original_inline_metadata_without_state_growth() {
    #[allow(dead_code)]
    enum Original {
        Preparation(diagnostic::Phase),
        Failure(diagnostic::Failure),
        Connection(diagnostic::Connection),
        MainFailure(diagnostic::MainFailure),
    }
    assert_eq!(size_of::<Observation>(), size_of::<Original>());
    assert_eq!(align_of::<Observation>(), align_of::<Original>());
}
