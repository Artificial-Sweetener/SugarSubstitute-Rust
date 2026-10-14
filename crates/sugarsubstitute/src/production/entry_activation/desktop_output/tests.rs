// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only byte chunks and logger writer/time boundaries; parsed facts grant no authority.
use super::*;
const LINE: &str = "operation=\"child_preparation_transition\" capture_admitted=true capture=false registered=false configuration=false original_failure=None capture_failure=None failure_retained=false";
#[cfg(not(feature = "k1-native-qualification"))]
#[test]
fn default_build_discards_compact_qualification_records() {
    let mut owner = Diagnostic::new();
    let bytes = b"INFO sugarsubstitute_desktop::qualification_trace: operation=\"child_native_qualification\" schema=1 stage=MainRevealed main=true launch=1 navigation=1\n";
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stderr, 1, false, false, bytes, |value| seen.push(value));
    assert!(seen.is_empty());
}
#[test]
fn closed_fields_accept_all_known_issues_but_reject_extensions_duplicates_and_unknown_values() {
    for issue in [
        "None",
        "Some(State)",
        "Some(Identity)",
        "Some(Transport)",
        "Some(Capture)",
        "Some(Configuration)",
        "Some(Deadline)",
    ] {
        assert!(
            parse(
                LINE.replace("original_failure=None", &format!("original_failure={issue}"))
                    .as_bytes()
            )
            .is_some()
        );
    }
    for class in [
        "None",
        "State",
        "Identity",
        "Admission",
        "Native",
        "Data",
        "Source",
        "TimedOut",
        "Cancelled",
        "Uncertain",
        "Other",
    ] {
        let actual = parse(
            LINE.replace("capture_failure=None", &format!("capture_failure={class}")).as_bytes(),
        );
        assert_eq!(actual.map(|phase| phase.capture_failure), Some(class));
    }
    for bad in [
        LINE.replace("capture_failure=None", "capture_failure=Native(secret)"),
        LINE.replace("capture_failure=None", "capture_failure=Unknown"),
        LINE.replace("capture_failure=None", "capture_failure=0xC0000005"),
        LINE.replace("capture_failure=None", "capture_failure=None capture_failure=Identity"),
        format!("{LINE} credential=private"),
        LINE.replace("original_failure=None", "original_failure=Some(Other)"),
        LINE.replace("capture=false", "capture=yes"),
        LINE.replace("registered=false", "capture=false"),
        format!("operation=\"other\" {LINE}"),
    ] {
        assert!(parse(bad.as_bytes()).is_none());
    }
}
#[test]
fn split_complete_stderr_line_is_decoded_once_and_stdout_remains_opaque() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stdout, 1, false, false, format!("{LINE}\n").as_bytes(), |v| {
        seen.push(v)
    });
    let at = LINE.len() / 2;
    owner.feed(OutputStream::Stderr, 1, false, false, &LINE.as_bytes()[..at], |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(
        OutputStream::Stderr,
        2,
        false,
        false,
        format!("{}\r\n", &LINE[at..]).as_bytes(),
        |v| seen.push(v),
    );
    owner.feed(OutputStream::Stderr, 3, false, false, format!("{LINE}\n").as_bytes(), |v| {
        seen.push(v)
    });
    assert_eq!(seen.len(), 1);
    assert!(matches!(
        seen[0],
        Observation::Preparation(Phase { capture_admitted: true, capture: false, .. })
    ));
}
#[test]
fn gaps_replays_and_eof_cannot_stitch_partial_lines_or_emit_partial_facts() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let at = LINE.len() / 2;
    owner.feed(OutputStream::Stderr, 1, false, false, &LINE.as_bytes()[..at], |v| seen.push(v));
    owner.feed(
        OutputStream::Stderr,
        3,
        true,
        false,
        format!("{}\n", &LINE[at..]).as_bytes(),
        |v| seen.push(v),
    );
    owner.feed(OutputStream::Stderr, 3, false, false, format!("{LINE}\n").as_bytes(), |v| {
        seen.push(v)
    });
    owner.feed(OutputStream::Stderr, 4, false, true, LINE.as_bytes(), |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 5, false, true, format!("{LINE}\n").as_bytes(), |v| {
        seen.push(v)
    });
    assert_eq!(seen.len(), 1);
}
#[test]
fn oversized_utf8_and_arbitrary_payloads_are_discarded_without_suffix_recovery() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let mut bad = vec![b'x'; 513];
    bad.extend_from_slice(format!("{LINE}\n").as_bytes());
    bad.extend_from_slice(b"credentials password secret\n\xff");
    bad.extend_from_slice(format!("{LINE}\n{LINE}\n").as_bytes());
    owner.feed(OutputStream::Stderr, 1, false, false, &bad, |v| seen.push(v));
    assert_eq!(seen.len(), 1);
}
#[test]
fn changing_diagnostics_remain_bounded_for_the_same_output_lifetime() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    for sequence in 1..=80 {
        let line = if sequence % 2 == 0 {
            LINE.replace("original_failure=None", "original_failure=Some(Deadline)")
        } else {
            LINE.to_owned()
        };
        owner.feed(
            OutputStream::Stderr,
            sequence,
            false,
            false,
            format!("{line}\n").as_bytes(),
            |v| seen.push(v),
        );
    }
    assert_eq!(seen.len(), 30);
    let failure = b"operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other\n";
    owner.feed(OutputStream::Stderr, 81, false, false, failure, |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 82, false, false, failure, |v| seen.push(v));
    assert_eq!(seen.len(), 31);
    assert!(matches!(
        seen.last(),
        Some(Observation::Failure(Failure {
            boundary: "PollFrame",
            kind: "Other",
            startup: None,
            consume: None
        }))
    ));
    let main = b"operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Frame code=JsonData\n";
    owner.feed(OutputStream::Stderr, 83, false, false, main, |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 84, false, false, main, |v| seen.push(v));
    assert_eq!(seen.len(), 32);
    assert!(matches!(seen.last(), Some(Observation::MainFailure(_))));
}
/// Use the production text formatter with deterministic external writer/time boundaries.
pub(in super::super) fn formatted_owner_diagnostics(
    emit: impl FnOnce(),
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
        type Writer = Sink;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let mut sink =
                self.0.lock().map_err(|_| std::io::Error::other("Writer fixture poisoned"))?;
            if sink.len().saturating_add(bytes.len()) > 32768 {
                return Err(std::io::Error::other("Writer fixture capacity"));
            }
            sink.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    struct Timestamp;
    impl tracing_subscriber::fmt::time::FormatTime for Timestamp {
        fn format_time(
            &self,
            w: &mut tracing_subscriber::fmt::format::Writer<'_>,
        ) -> std::fmt::Result {
            write!(w, "2026-10-13T00:00:00.000000Z")
        }
    }
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(Sink(bytes.clone()))
        .with_timer(Timestamp)
        .with_ansi(false)
        .finish();
    tracing::dispatcher::with_default(&tracing::Dispatch::new(subscriber), emit);
    let actual =
        bytes.lock().map_err(|_| std::io::Error::other("Writer fixture poisoned"))?.clone();
    Ok(actual)
}

#[test]
fn actual_production_text_formatter_with_writer_and_time_boundaries_preserves_closed_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = formatted_owner_diagnostics(|| {
        tracing::info!(operation="child_preparation_transition",capture_admitted=true,capture=false,
            registered=false,configuration=false,original_failure=?None::<u8>,capture_failure=%"None",failure_retained=false,
            "Child pre-UI preparation conditions changed");
        for stage in [Connection::Queued, Connection::Admitted, Connection::ReplyReceived] {
            tracing::info!(operation="child_connection_transition",stage=?stage,
                "Original child connection request advanced");
        }
        tracing::warn!(operation="desktop_failure_transition",boundary=%"ObserveStartup",kind=%"Other",
            observation=%"CloseRequested",close_stage=?sugarsubstitute_desktop::StartupCloseStage::CaptureObservation,
            capture_snapshot=?sugarsubstitute_desktop::StartupCaptureState::CounterFailed,
            code=%sugarsubstitute_desktop::StartupCauseCode::Native(sugarsubstitute_desktop::StartupNativeCause::CounterConversion,Some(u32::MAX)),
            "First owned desktop failure retained");
        tracing::warn!(operation="main_failure_transition",failed_operation="decode_main_observation",
            launch=u64::MAX,navigation=u64::MAX,observation=%"Frame",code=?main::Code::Hresult(i32::MIN),
            "Original owned main failure transferred to startup policy");
    })?;
    assert!(bytes.starts_with(b"2026-10-13T"));
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stderr, 1, false, false, &bytes, |v| seen.push(v));
    assert_eq!(seen.len(), 6);
    assert!(bytes.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
    assert_eq!(parse(LINE.as_bytes()).map(Observation::Preparation), seen.first().copied());
    assert_eq!(
        &seen[1..4],
        &[
            Observation::Connection(Connection::Queued),
            Observation::Connection(Connection::Admitted),
            Observation::Connection(Connection::ReplyReceived),
        ]
    );
    assert!(matches!(
        seen[4],
        Observation::Failure(Failure { boundary: "ObserveStartup", kind: "Other", startup: Some(detail), consume: None })
            if detail == (startup::StartupFailureDetail {
                observation: "CloseRequested",
                stage: sugarsubstitute_desktop::StartupCloseStage::CaptureObservation,
                capture: sugarsubstitute_desktop::StartupCaptureState::CounterFailed,
                code: sugarsubstitute_desktop::StartupCauseCode::Native(sugarsubstitute_desktop::StartupNativeCause::CounterConversion,Some(u32::MAX)),
            })
    ));
    assert_eq!(
        seen[5],
        Observation::MainFailure(MainFailure {
            launch: u64::MAX,
            navigation: u64::MAX,
            operation: "decode_main_observation",
            observation: "Frame",
            code: main::Code::Hresult(i32::MIN),
            timeout: None,
        })
    );
    Ok(())
}

#[test]
fn closed_desktop_failure_fields_reject_payloads_unknown_values_and_extra_operations() {
    for boundary in [
        "PollFrame",
        "RequestFrame",
        "ObserveStartup",
        "TakeCommand",
        "Visibility",
        "PreSurface",
        "Cleanup",
        "Presentation",
        "CursorMonitor",
        "MonitorWorkArea",
        "CenterWindow",
        "Other",
    ] {
        for kind in ["TimedOut", "Disconnected", "Denied", "InvalidInput", "Io", "Other"] {
            let line =
                format!("operation=\"desktop_failure_transition\" boundary={boundary} kind={kind}");
            assert_eq!(
                parse_failure(line.as_bytes()),
                Some(Failure { boundary, kind, startup: None, consume: None })
            );
        }
    }
    for line in [
        "operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other cause=PRIVATE",
        "operation=\"desktop_failure_transition\" boundary=secret kind=Other",
        "operation=\"desktop_failure_transition\" boundary=PollFrame kind=0x80004005",
        "operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other kind=Denied",
        "operation=\"other\" operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other",
    ] {
        assert!(parse_failure(line.as_bytes()).is_none());
    }
}

/// Exact progression rejects unknown fields, regressions and duplicates without using more slots.
#[test]
fn closed_connection_progress_rejects_extensions_replays_and_regressions() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let mut sequence = 0;
    for token in [
        "Admitted",
        "Queued",
        "Queued",
        "Unknown",
        "Queued payload=secret",
        "Admitted stage=ReplyReceived",
        "ReplyReceived",
        "Admitted",
        "Queued",
        "ReplyReceived",
        "ReplyReceived",
    ] {
        sequence += 1;
        let line = format!("operation=\"child_connection_transition\" stage={token}\n");
        owner.feed(OutputStream::Stderr, sequence, false, false, line.as_bytes(), |v| seen.push(v));
    }
    assert_eq!(
        seen,
        vec![
            Observation::Connection(Connection::Queued),
            Observation::Connection(Connection::Admitted),
            Observation::Connection(Connection::ReplyReceived)
        ]
    );
    assert!(
        parse_connection(
            b"operation=\"other\" operation=\"child_connection_transition\" stage=Queued"
        )
        .is_none()
    );
}

/// Connection fragments and shared saturation retain the unchanged first-failure reserve.
#[test]
fn connection_chunks_share_original_diagnostic_cap_and_cannot_stitch_gaps() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let queued = b"operation=\"child_connection_transition\" stage=Queued\n";
    let at = queued.len() / 2;
    owner.feed(OutputStream::Stderr, 1, false, false, &queued[..at], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 3, true, false, &queued[at..], |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 4, false, false, &queued[..at], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 5, false, false, &queued[at..], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 5, false, false, queued, |v| seen.push(v));
    assert_eq!(seen, vec![Observation::Connection(Connection::Queued)]);
    for sequence in 6..70 {
        let capture = sequence % 2 == 0;
        let line = format!("{}\n", LINE.replace("capture=false", &format!("capture={capture}")));
        owner.feed(OutputStream::Stderr, sequence, false, false, line.as_bytes(), |v| seen.push(v));
    }
    owner.feed(
        OutputStream::Stderr,
        70,
        false,
        false,
        b"operation=\"child_connection_transition\" stage=Admitted\n",
        |v| seen.push(v),
    );
    owner.feed(
        OutputStream::Stderr,
        71,
        false,
        false,
        b"operation=\"desktop_failure_transition\" boundary=PollFrame kind=TimedOut\n",
        |v| seen.push(v),
    );
    assert_eq!(seen.len(), 31);
    assert!(matches!(seen.last(), Some(Observation::Failure(Failure { kind: "TimedOut", .. }))));
}

#[test]
fn terminal_acquisition_metadata_requires_complete_order_and_rejects_extra_raw_fields() {
    for terminal in [Connection::Granted, Connection::Failed, Connection::Rejected] {
        let mut parser = Diagnostic::new();
        let mut observed = Vec::new();
        let premature = format!("operation=\"child_connection_transition\" stage={terminal:?}\n");
        parser.feed(OutputStream::Stderr, 1, false, false, premature.as_bytes(), |fact| {
            observed.push(fact)
        });
        assert!(observed.is_empty());
        for (index, stage) in
            [Connection::Queued, Connection::Admitted, Connection::ReplyReceived, terminal]
                .into_iter()
                .enumerate()
        {
            let line = format!("operation=\"child_connection_transition\" stage={stage:?}\n");
            parser.feed(
                OutputStream::Stderr,
                index as u64 + 2,
                false,
                false,
                line.as_bytes(),
                |fact| observed.push(fact),
            );
        }
        assert_eq!(observed.len(), 4);
        assert!(
            matches!(observed.last(), Some(Observation::Connection(actual)) if *actual == terminal)
        );
        parser.feed(OutputStream::Stderr, 6, false, true, premature.as_bytes(), |fact| {
            observed.push(fact)
        });
        assert_eq!(observed.len(), 4);
        let extension = format!(
            "operation=\"child_connection_transition\" stage={terminal:?} credential=opaque"
        );
        assert!(parse_connection(extension.as_bytes()).is_none());
    }
}
