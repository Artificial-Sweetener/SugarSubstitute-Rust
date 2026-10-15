// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact closed lifecycle diagnostics; output ownership supplies authentic run correlation.
use sugarsubstitute_application::startup::StartupPhase;
/// Identify the finite exit controls without exposing their payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Control {
    MainRetired,
    Release,
    ExitIntent,
    ExitCompleted,
}
/// Distinguish preparation, native admission and actual reply consumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControlStage {
    Prepared,
    Admitted,
    ReplyReceived,
    ReplyConsumed,
}
/// Name the owner supplying the unchanged control cutoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeadlineSource {
    ShutdownGrace,
    ShutdownTotal,
    AttemptCleanup,
    ControlInterval,
}
/// Describe the actual selected original cutoff without supplying authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Prepared {
    pub(super) original_ns: u64,
    pub(super) source: DeadlineSource,
    pub(super) phase: Option<StartupPhase>,
    pub(super) attempt_present: bool,
    pub(super) attempt_complete: bool,
    pub(super) shutdown_budget_present: bool,
}
/// Describe the first capture or local shutdown measurement transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BudgetStage {
    CapturePending,
    LocalMeasured,
    MeasurementFailed,
    CaptureLatched,
    CaptureMeasurementFailed,
}
/// Fixed closed metadata is inert; the authenticated output owner supplies its Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CloseTrace {
    Control {
        stage: ControlStage,
        control: Control,
        request: u64,
        deadline_qpc: i64,
        prepared: Option<Prepared>,
    },
    Budget {
        stage: BudgetStage,
        moments: Option<[u64; 3]>,
    },
}
impl CloseTrace {
    /// Emit compact scalars through the existing child's bounded stderr channel.
    pub(super) fn emit(self) {
        match self {
            Self::Control { stage, control, request, deadline_qpc, prepared: Some(p) } => {
                tracing::info!(operation="child_close_control",stage=?stage,control=?control,request,deadline_qpc,
                    original_ns=p.original_ns,deadline_source=?p.source,phase=?p.phase,
                    attempt_present=p.attempt_present,attempt_complete=p.attempt_complete,shutdown_budget_present=p.shutdown_budget_present);
            }
            Self::Control { stage, control, request, deadline_qpc, prepared: None } => {
                tracing::info!(operation="child_close_control",stage=?stage,control=?control,request,deadline_qpc);
            }
            Self::Budget { stage, moments: Some([started_ns, graceful_ns, settle_ns]) } => {
                tracing::info!(operation="child_shutdown_budget",stage=?stage,started_ns,graceful_ns,settle_ns);
            }
            Self::Budget { stage, moments: None } => {
                tracing::info!(operation="child_shutdown_budget",stage=?stage);
            }
        }
    }
}
