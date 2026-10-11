// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve raw native qualification counters without reducing or hiding observations.

use serde_json::{Value, json};
use sugarsubstitute_desktop::{NativeCallbackKind, NativeTimingReceipt, NativeTimingSource};

/// Serialize native timing evidence after teardown, keeping omissions and frame identity explicit.
pub(crate) fn encode(receipt: NativeTimingReceipt) -> Value {
    let callbacks: Vec<_> = receipt
        .callbacks
        .iter()
        .map(|sample| {
            json!({
                "sequence": sample.sequence,
                "kind": callback_kind(sample.kind),
                "begin_ticks": sample.begin_ticks,
                "end_ticks": sample.end_ticks,
                "authored_ticks": sample.authored_ticks,
                "delegated_drag": sample.delegated_drag.map(|interval| json!({
                    "begin_ticks": interval.begin_ticks,
                    "end_ticks": interval.end_ticks,
                })),
            })
        })
        .collect();
    let submissions: Vec<_> = receipt
        .submissions
        .iter()
        .map(|sample| {
            json!({
                "sequence": sample.sequence,
                "begin_ticks": sample.begin_ticks,
                "end_ticks": sample.end_ticks,
                "retained": sample.retained,
                "identity": {
                    "surface": sample.identity.surface.get(),
                    "state_generation": sample.identity.state_generation.get(),
                    "sequence": sample.identity.sequence.get(),
                },
            })
        })
        .collect();
    json!({
        "schema_version": 1,
        "source": match receipt.source { NativeTimingSource::WindowsQpc => "windows_qpc" },
        "begin_ticks": receipt.begin_ticks,
        "end_ticks": receipt.end_ticks,
        "frequency": receipt.frequency,
        "ui_thread_id": receipt.ui_thread_id,
        "callbacks_observed": receipt.callbacks_observed,
        "callback_samples_omitted": receipt.callback_samples_omitted,
        "submissions_observed": receipt.submissions_observed,
        "submission_samples_omitted": receipt.submission_samples_omitted,
        "callbacks": callbacks,
        "submissions": submissions,
    })
}

/// Keep event categories stable for qualification analysis instead of relying on debug spelling.
fn callback_kind(kind: NativeCallbackKind) -> &'static str {
    match kind {
        NativeCallbackKind::WindowsParent => "windows_parent",
        NativeCallbackKind::WindowsChild => "windows_child",
        NativeCallbackKind::WindowEvent => "window_event",
        NativeCallbackKind::CompletionDrain => "completion_drain",
        NativeCallbackKind::NativeAction => "native_action",
        NativeCallbackKind::Redraw => "redraw",
        NativeCallbackKind::LoopProgress => "loop_progress",
        NativeCallbackKind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use sugarsubstitute_contracts::{
        execution::{Generation, SurfaceId},
        presentation::FrameIdentity,
    };
    use sugarsubstitute_desktop::{
        NativeCallbackTiming, NativeSubmissionTiming, NativeTickInterval,
    };

    /// Keep raw integers, overlapping drag evidence, omissions and actual producer identity.
    #[test]
    fn serialization_preserves_native_evidence_without_hiding_omissions()
    -> Result<(), Box<dyn Error>> {
        let encoded = encode(NativeTimingReceipt {
            source: NativeTimingSource::WindowsQpc,
            begin_ticks: 9_007_199_254_740_992,
            end_ticks: 9_007_199_254_741_004,
            frequency: 10_000_000,
            ui_thread_id: 17,
            callbacks_observed: 2,
            callback_samples_omitted: 1,
            submissions_observed: 3,
            submission_samples_omitted: 2,
            callbacks: vec![NativeCallbackTiming {
                sequence: 1,
                kind: NativeCallbackKind::WindowsParent,
                begin_ticks: 9_007_199_254_740_993,
                end_ticks: 9_007_199_254_741_003,
                authored_ticks: 4,
                delegated_drag: Some(NativeTickInterval {
                    begin_ticks: 9_007_199_254_740_995,
                    end_ticks: 9_007_199_254_741_001,
                }),
            }],
            submissions: vec![NativeSubmissionTiming {
                sequence: 1,
                begin_ticks: 9_007_199_254_740_996,
                end_ticks: 9_007_199_254_740_999,
                retained: true,
                identity: FrameIdentity {
                    surface: SurfaceId::new(7)?,
                    state_generation: Generation::new(11)?,
                    sequence: Generation::new(13)?,
                },
            }],
        });
        let roundtrip: Value = serde_json::from_slice(&serde_json::to_vec(&encoded)?)?;
        assert_eq!(roundtrip["callbacks"][0]["begin_ticks"].as_i64(), Some(9_007_199_254_740_993));
        assert_eq!(roundtrip["callbacks"][0]["authored_ticks"], 4);
        assert_eq!(
            roundtrip["callbacks"][0]["delegated_drag"]["end_ticks"].as_i64(),
            Some(9_007_199_254_741_001)
        );
        assert_eq!(roundtrip["callback_samples_omitted"], 1);
        assert_eq!(roundtrip["submission_samples_omitted"], 2);
        assert_eq!(roundtrip["submissions"][0]["retained"], true);
        assert_eq!(roundtrip["submissions"][0]["identity"]["sequence"], 13);
        assert_eq!(roundtrip["source"], "windows_qpc");
        Ok(())
    }
}
