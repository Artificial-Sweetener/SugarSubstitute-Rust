// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify closed console diagnostics through actual formatting, framing and the Root relay.
use super::super::{Diagnostic, Failure, Observation, failure};
use super::*;
use sugarsubstitute_contracts::{lifecycle::*, transcript::OutputStream};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const LINE: &str = "operation=\"desktop_failure_transition\" boundary=Other kind=Other console_operation=ScrollObserve console_code=Hresult(-2147023449)";
fn bytes(emit: impl FnOnce()) -> Result<Vec<u8>> {
    super::super::tests::formatted_owner_diagnostics(emit)
}
fn run() -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
#[test]
fn console_decoder_accepts_every_direct_code_with_all_legacy_kinds() {
    for kind in ["Other", "Io", "TimedOut", "Disconnected", "Denied", "InvalidInput"] {
        for code in [
            "Opaque",
            "IntegerRange",
            "Allocation",
            "MemoryClaim",
            "Io(Io)",
            "Io(TimedOut)",
            "Io(Disconnected)",
            "Io(Denied)",
            "Io(InvalidInput)",
            "Os(-2147483648)",
            "Os(0)",
            "Os(2147483647)",
            "Hresult(-2147483648)",
            "Hresult(0)",
            "Hresult(2147483647)",
        ] {
            let line = LINE
                .replace("kind=Other", &format!("kind={kind}"))
                .replace("Hresult(-2147023449)", code);
            let actual = failure::parse(line.as_bytes());
            assert!(
                matches!(
                    actual,
                    Some(Failure {
                        boundary: "Other",
                        startup: None,
                        consume: None,
                        console: Some(_),
                        ..
                    })
                ),
                "rejected {line}"
            );
            assert_eq!(actual.map(|f| f.kind), Some(kind));
        }
    }
}
#[test]
fn console_decoder_rejects_missing_unknown_duplicate_mixed_and_hostile_fields() {
    for bad in [
        LINE.replace("ScrollObserve", "PRIVATE"),
        LINE.replace("ScrollObserve", "console_scroll_observe"),
        LINE.replace("console_operation=ScrollObserve ", ""),
        LINE.replace(" console_code=Hresult(-2147023449)", ""),
        LINE.replace("kind=Other", "kind=PRIVATE"),
        LINE.replace("boundary=Other", "boundary=ObserveStartup"),
        LINE.replace("boundary=Other", "boundary=PollFrame"),
        LINE.replace("console_code=", "category="),
        LINE.replace("console_operation=", "failed_operation="),
        format!("{LINE} console_operation=LibraryLoad"),
        format!("{LINE} console_code=Opaque"),
        format!("{LINE} observation=CloseRequested"),
        format!("{LINE} consume_stage=ChildDelivery"),
        format!("{LINE} cause=PRIVATE"),
        format!("operation=\"foreign\" {LINE}"),
        format!("{LINE} {LINE}"),
        LINE.replace(
            "console_operation=ScrollObserve console_code=Hresult(-2147023449)",
            "console_code=Opaque console_operation=ScrollObserve",
        ),
        LINE.replace("Hresult(-2147023449)", "Io(Other)"),
        LINE.replace("Hresult(-2147023449)", "JsonData"),
        LINE.replace("Hresult(-2147023449)", "Native(Query)"),
    ] {
        assert!(failure::parse(bad.as_bytes()).is_none(), "accepted {bad}");
    }
    for raw in [
        "+1",
        "-0",
        "01",
        "-01",
        "",
        "--1",
        "2147483648",
        "-2147483649",
        "0x80070005",
        "NaN",
        "1,2",
        "1)",
    ] {
        for variant in ["Hresult", "Os"] {
            let bad = LINE.replace("Hresult(-2147023449)", &format!("{variant}({raw})"));
            assert!(failure::parse(bad.as_bytes()).is_none(), "accepted {bad}");
        }
    }
    assert!(failure::parse(b"operation=\"desktop_failure_transition\" boundary=Other kind=Other console_operation=ScrollObserve console_code=Opaque\xff").is_none());
}
#[test]
fn actual_console_formatter_decodes_fragmented_and_relays_the_exact_operation_code_and_run()
-> Result {
    let formatted = bytes(|| {
        tracing::warn!(operation="desktop_failure_transition",boundary=%"Other",kind=%"Other",
            console_operation=?Operation::ScrollObserve,console_code=?Code::Hresult(-2147023449),
            "First owned desktop failure retained");
    })?;
    let text = std::str::from_utf8(&formatted)?;
    assert!(text.ends_with(&format!("First owned desktop failure retained {LINE}\n")));
    assert!(formatted.len() - 1 <= 512);
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    for (index, fragment) in formatted.chunks(7).enumerate() {
        owner
            .feed(OutputStream::Stderr, index as u64 + 1, false, false, fragment, |v| seen.push(v));
    }
    let expected = Failure {
        boundary: "Other",
        kind: "Other",
        startup: None,
        consume: None,
        console: Some(ConsoleFailureDetail {
            operation: Operation::ScrollObserve,
            code: Code::Hresult(-2147023449),
        }),
    };
    assert_eq!(seen, vec![Observation::Failure(expected)]);
    let identity = run()?;
    let relayed = String::from_utf8(bytes(|| failure::relay(identity, expected))?)?;
    assert!(relayed.contains("sugarsubstitute::production::entry_activation::desktop_output:"));
    assert!(relayed.ends_with(&format!("Observed first closed owned desktop failure operation=\"desktop_child_failure\" run={identity:?} boundary=Other kind=Other console_operation=ScrollObserve console_code=Hresult(-2147023449)\n")));
    assert!(!relayed.contains("PRIVATE"));
    Ok(())
}
#[test]
fn console_line_boundary_is_exact_and_first_failure_keeps_its_reserved_slot() {
    let exact = format!("{}{LINE}", "x".repeat(512 - LINE.len()));
    assert_eq!(exact.len(), 512);
    assert!(failure::parse(exact.as_bytes()).is_some());
    assert!(failure::parse(format!("x{exact}").as_bytes()).is_none());
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let progress = "operation=\"child_preparation_transition\" channel_phase=Authenticated channel_terminal=None capture_input=Transferred capture_admitted=true capture=false registered=false configuration=false original_failure=None capture_failure=None failure_retained=";
    for sequence in 1..=80 {
        let line = format!("{progress}{}\n", sequence % 2 == 0);
        owner.feed(OutputStream::Stderr, sequence, false, false, line.as_bytes(), |v| seen.push(v));
    }
    owner.feed(OutputStream::Stderr, 81, false, false, format!("{exact}\n").as_bytes(), |v| {
        seen.push(v)
    });
    owner.feed(OutputStream::Stderr, 82, false, false, format!("{LINE}\n").as_bytes(), |v| {
        seen.push(v)
    });
    assert_eq!(seen.len(), 31);
    assert!(matches!(seen.last(), Some(Observation::Failure(Failure { console: Some(_), .. }))));
    let main = b"operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Frame code=Opaque\n";
    owner.feed(OutputStream::Stderr, 83, false, false, main, |v| seen.push(v));
    assert_eq!(seen.len(), 32);
    assert!(matches!(seen.last(), Some(Observation::MainFailure(_))));
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(
        OutputStream::Stderr,
        1,
        false,
        false,
        format!("x{exact}\n{LINE}\n").as_bytes(),
        |v| seen.push(v),
    );
    assert_eq!(seen.len(), 1);
}
#[test]
fn console_stream_loss_stdout_and_partial_eof_cannot_consume_the_first_failure_slot() {
    let full = format!("{LINE}\n");
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stdout, 1, false, false, full.as_bytes(), |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 1, false, false, &full.as_bytes()[..40], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 3, false, false, &full.as_bytes()[40..], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 4, false, true, &full.as_bytes()[..40], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 5, false, false, &full.as_bytes()[40..], |v| seen.push(v));
    owner.feed(
        OutputStream::Stderr,
        6,
        false,
        false,
        full.replace("ScrollObserve", "PRIVATE").as_bytes(),
        |v| seen.push(v),
    );
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 7, false, false, full.as_bytes(), |v| seen.push(v));
    assert_eq!(seen.len(), 1);
}
#[test]
fn legacy_failure_decoding_and_relay_keep_their_original_fields_without_console_metadata() -> Result
{
    for (boundary, kind) in [("PollFrame", "Other"), ("Other", "Io"), ("Visibility", "Denied")] {
        let line =
            format!("operation=\"desktop_failure_transition\" boundary={boundary} kind={kind}");
        let actual = failure::parse(line.as_bytes()).ok_or("Legacy record rejected")?;
        assert_eq!(actual, Failure { boundary, kind, startup: None, consume: None, console: None });
        let identity = run()?;
        let relay = String::from_utf8(bytes(|| failure::relay(identity, actual))?)?;
        assert!(relay.ends_with(&format!("Observed first closed owned desktop failure operation=\"desktop_child_failure\" run={identity:?} boundary={boundary} kind={kind}\n")));
        assert!(!relay.contains("console_"));
    }
    for line in [
        "operation=\"desktop_failure_transition\" boundary=ObserveStartup kind=Other observation=CloseRequested close_stage=CaptureObservation capture_snapshot=CounterFailed code=Native(CounterConversion)",
        "operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other consume_stage=ChildDelivery category=Bootstrap(Transport)",
    ] {
        let actual = failure::parse(line.as_bytes()).ok_or("Legacy enriched record rejected")?;
        assert!(actual.console.is_none());
    }
    Ok(())
}
