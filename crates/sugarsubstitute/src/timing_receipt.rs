// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve raw native qualification counters without reducing or hiding observations.

use serde_json::{Value, json};
use sugarsubstitute_desktop::{
    NativeCallbackKind, NativeStartupOrder, NativeStartupStage, NativeTimingReceipt,
    NativeTimingSource,
};

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
    let startup_milestones: Vec<_> = receipt
        .startup_milestones
        .iter()
        .flatten()
        .map(|sample| json!({"stage": startup_stage(sample.stage), "ticks": sample.ticks}))
        .collect();
    json!({
        "schema_version": 4,
        "startup_order": match receipt.startup_order {
            NativeStartupOrder::SurfaceFirst => "surface_first",
            NativeStartupOrder::ExecutionFirst => "execution_first",
        },
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
        "startup_milestones": startup_milestones,
    })
}

/// Keep the seven startup boundaries stable without serializing enum debug spelling.
fn startup_stage(stage: NativeStartupStage) -> &'static str {
    match stage {
        NativeStartupStage::EventLoopReady => "event_loop_ready",
        NativeStartupStage::WindowCreated => "window_created",
        NativeStartupStage::PresenterReady => "presenter_ready",
        NativeStartupStage::ControlsReady => "controls_ready",
        NativeStartupStage::ExecutionReady => "execution_ready",
        NativeStartupStage::ShowFocusReturned => "show_focus_returned",
        NativeStartupStage::FirstPreparationRequest => "first_preparation_request",
    }
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
        NativeCallbackTiming, NativeStartupTiming, NativeSubmissionTiming, NativeTickInterval,
    };

    /// Keep raw integers, overlapping drag evidence, omissions and actual producer identity.
    #[test]
    fn serialization_preserves_native_evidence_without_hiding_omissions()
    -> Result<(), Box<dyn Error>> {
        let encoded = encode(NativeTimingReceipt {
            source: NativeTimingSource::WindowsQpc,
            startup_order: NativeStartupOrder::SurfaceFirst,
            begin_ticks: 9_007_199_254_740_992,
            end_ticks: 9_007_199_254_741_004,
            frequency: 10_000_000,
            ui_thread_id: 17,
            callbacks_observed: 2,
            callback_samples_omitted: 1,
            submissions_observed: 3,
            submission_samples_omitted: 2,
            startup_milestones: [
                NativeStartupStage::EventLoopReady,
                NativeStartupStage::WindowCreated,
                NativeStartupStage::PresenterReady,
                NativeStartupStage::ControlsReady,
                NativeStartupStage::ExecutionReady,
                NativeStartupStage::FirstPreparationRequest,
                NativeStartupStage::ShowFocusReturned,
            ]
            .map(|stage| Some(NativeStartupTiming { stage, ticks: 9_007_199_254_740_994 })),
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
                begin_ticks: 9_007_199_254_741_002,
                end_ticks: 9_007_199_254_741_003,
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
        assert_eq!(roundtrip["schema_version"], 4);
        assert_eq!(roundtrip["startup_order"], "surface_first");
        assert_eq!(
            roundtrip["startup_milestones"].as_array().ok_or("Milestones missing")?.len(),
            7
        );
        assert_eq!(roundtrip["startup_milestones"][0]["stage"], "event_loop_ready");
        assert_eq!(roundtrip["startup_milestones"][5]["stage"], "first_preparation_request");
        assert_eq!(roundtrip["startup_milestones"][6]["stage"], "show_focus_returned");
        assert_eq!(
            roundtrip["startup_milestones"][6]["ticks"].as_i64(),
            Some(9_007_199_254_740_994)
        );
        Ok(())
    }

    /// Keep failed/partial native setup evidence as an ordered prefix without fabricated stages.
    #[test]
    fn partial_startup_receipt_serializes_only_actual_observed_stages() {
        let mut milestones = [None; 7];
        milestones[0] = Some(NativeStartupTiming {
            stage: NativeStartupStage::EventLoopReady,
            ticks: 9_007_199_254_740_993,
        });
        let encoded = encode(NativeTimingReceipt {
            source: NativeTimingSource::WindowsQpc,
            startup_order: NativeStartupOrder::SurfaceFirst,
            begin_ticks: 9_007_199_254_740_992,
            end_ticks: 9_007_199_254_740_994,
            frequency: 10_000_000,
            ui_thread_id: 7,
            callbacks: Vec::new(),
            submissions: Vec::new(),
            startup_milestones: milestones,
            callbacks_observed: 0,
            callback_samples_omitted: 0,
            submissions_observed: 0,
            submission_samples_omitted: 0,
        });
        assert_eq!(encoded["schema_version"], 4);
        assert_eq!(
            encoded["startup_milestones"],
            json!([
                {"stage":"event_loop_ready", "ticks":9_007_199_254_740_993_i64}
            ])
        );
        assert_eq!(encoded["submissions"], json!([]));
    }

    /// Persist actual execution-first observations explicitly without sorting them into old order.
    #[test]
    fn execution_first_receipt_preserves_its_exact_mode_and_observed_prefix() {
        let mut milestones = [None; 7];
        for (index, stage) in
            NativeStartupOrder::ExecutionFirst.stages().into_iter().take(2).enumerate()
        {
            milestones[index] = Some(NativeStartupTiming { stage, ticks: 10 + index as i64 });
        }
        let encoded = encode(NativeTimingReceipt {
            source: NativeTimingSource::WindowsQpc,
            startup_order: NativeStartupOrder::ExecutionFirst,
            begin_ticks: 9,
            end_ticks: 12,
            frequency: 1000,
            ui_thread_id: 7,
            callbacks: Vec::new(),
            submissions: Vec::new(),
            startup_milestones: milestones,
            callbacks_observed: 0,
            callback_samples_omitted: 0,
            submissions_observed: 0,
            submission_samples_omitted: 0,
        });
        assert_eq!(encoded["schema_version"], 4);
        assert_eq!(encoded["startup_order"], "execution_first");
        assert_eq!(
            encoded["startup_milestones"],
            json!([
                {"stage":"execution_ready","ticks":10},{"stage":"event_loop_ready","ticks":11}
            ])
        );
    }
}
