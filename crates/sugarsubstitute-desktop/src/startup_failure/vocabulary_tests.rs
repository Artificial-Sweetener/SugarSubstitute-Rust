// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify every emitted failure-frame token decodes to exactly the value that produced it.
use super::*;

/// Each listed variant's token equals its emitted `Debug` form and decodes back to it alone.
fn round_trips<T: Copy + std::fmt::Debug + Eq>(
    all: &[T],
    token: impl Fn(T) -> &'static str,
    parse: impl Fn(&str) -> Option<T>,
) {
    for &value in all {
        let emitted = format!("{value:?}");
        assert_eq!(token(value), emitted);
        assert_eq!(parse(&emitted), Some(value), "{emitted}");
        assert_eq!(parse(&emitted.to_ascii_lowercase()), None, "{emitted}");
        assert_eq!(parse(&format!("{emitted} ")), None, "{emitted}");
        assert_eq!(parse(&format!("{emitted}x")), None, "{emitted}");
    }
    let mut tokens: Vec<_> = all.iter().map(|value| token(*value)).collect();
    tokens.sort_unstable();
    tokens.dedup();
    assert_eq!(tokens.len(), all.len());
}

#[test]
fn every_fieldless_failure_fact_decodes_from_its_emitted_form() {
    round_trips(StartupObservationKind::ALL, StartupObservationKind::token, |v| {
        StartupObservationKind::parse(v)
    });
    round_trips(StartupCanvasMessageCause::ALL, StartupCanvasMessageCause::token, |v| {
        StartupCanvasMessageCause::parse(v)
    });
    round_trips(StartupCloseStage::ALL, StartupCloseStage::token, StartupCloseStage::parse);
    round_trips(StartupCaptureState::ALL, StartupCaptureState::token, StartupCaptureState::parse);
    round_trips(StartupNativeCause::ALL, StartupNativeCause::token, StartupNativeCause::parse);
    round_trips(StartupRequestCause::ALL, StartupRequestCause::token, StartupRequestCause::parse);
    round_trips(StartupBootstrapCause::ALL, StartupBootstrapCause::token, |v| {
        StartupBootstrapCause::parse(v)
    });
    round_trips(StartupConsumeStage::ALL, StartupConsumeStage::token, StartupConsumeStage::parse);
    assert_eq!(StartupObservationKind::CanvasNativeMessage.to_string(), "CanvasNativeMessage");
}

/// One value of every cause-code shape; the exhaustive match fails to compile when a shape is
/// added without a decoded sample here.
fn cause_samples() -> Vec<StartupCauseCode> {
    let mut samples = vec![
        StartupCauseCode::Opaque,
        StartupCauseCode::Os(0),
        StartupCauseCode::Os(i32::MIN),
        StartupCauseCode::Hresult(-2_147_023_449),
        StartupCauseCode::TimedOut,
        StartupCauseCode::Disconnected,
        StartupCauseCode::Denied,
        StartupCauseCode::InvalidInput,
        StartupCauseCode::Io,
    ];
    for &stage in StartupNativeCause::ALL {
        samples.extend([
            StartupCauseCode::Native(stage, None),
            StartupCauseCode::Native(stage, Some(u32::MAX)),
        ]);
    }
    samples.extend(StartupRequestCause::ALL.iter().map(|cause| StartupCauseCode::Request(*cause)));
    samples.extend(StartupBootstrapCause::ALL.iter().map(|v| StartupCauseCode::Bootstrap(*v)));
    samples.extend(
        StartupCanvasMessageCause::ALL.iter().map(|cause| StartupCauseCode::CanvasMessage(*cause)),
    );
    for transcript in [
        TranscriptError::ZeroIdentity,
        TranscriptError::Bound,
        TranscriptError::EmptyGap,
        TranscriptError::InvalidStyle,
        TranscriptError::ForeignResource,
        TranscriptError::Sequence,
        TranscriptError::StreamEnded,
        TranscriptError::Exhausted,
        TranscriptError::SnapshotCapacity,
    ] {
        // Adding a transcript rejection requires a sample above.
        match transcript {
            TranscriptError::ZeroIdentity
            | TranscriptError::Bound
            | TranscriptError::EmptyGap
            | TranscriptError::InvalidStyle
            | TranscriptError::ForeignResource
            | TranscriptError::Sequence
            | TranscriptError::StreamEnded
            | TranscriptError::Exhausted
            | TranscriptError::SnapshotCapacity => {}
        }
        samples.push(StartupCauseCode::Transcript(transcript));
    }
    for sample in &samples {
        match sample {
            StartupCauseCode::Opaque
            | StartupCauseCode::Os(_)
            | StartupCauseCode::Hresult(_)
            | StartupCauseCode::TimedOut
            | StartupCauseCode::Disconnected
            | StartupCauseCode::Denied
            | StartupCauseCode::InvalidInput
            | StartupCauseCode::Io
            | StartupCauseCode::Native(..)
            | StartupCauseCode::Request(_)
            | StartupCauseCode::Bootstrap(_)
            | StartupCauseCode::Transcript(_)
            | StartupCauseCode::CanvasMessage(_) => {}
        }
    }
    samples
}

#[test]
fn every_cause_code_decodes_from_its_emitted_form_and_noncanonical_text_is_refused() {
    for code in cause_samples() {
        let emitted = code.to_string();
        assert_eq!(StartupCauseCode::parse(&emitted), Some(code), "{emitted}");
        assert_eq!(StartupCauseCode::parse(&format!("{emitted} ")), None, "{emitted}");
    }
    for refused in [
        "",
        "opaque",
        "Os()",
        "Os(+1)",
        "Os(01)",
        "Os(-0)",
        "Os(2147483648)",
        "Hresult(1",
        "NativeOs(Other,)",
        "NativeOs(Other,-1)",
        "NativeOs(Other,01)",
        "NativeOs(Unknown,1)",
        "Native(Unknown)",
        "Request(Other)",
        "Bootstrap(Other)",
        "Transcript(Other)",
        "CanvasMessage(Other)",
        "CanvasMessage(Storage)x",
    ] {
        assert_eq!(StartupCauseCode::parse(refused), None, "{refused}");
    }
}

#[test]
fn a_refused_canvas_message_projects_its_closed_reason() {
    assert_eq!(
        StartupCauseCode::direct(&crate::CanvasNativeReplyError::Storage),
        StartupCauseCode::CanvasMessage(StartupCanvasMessageCause::Storage)
    );
    assert_eq!(
        StartupCauseCode::direct(&crate::CanvasNativeReplyError::ForeignNavigation).to_string(),
        "CanvasMessage(ForeignNavigation)"
    );
}
