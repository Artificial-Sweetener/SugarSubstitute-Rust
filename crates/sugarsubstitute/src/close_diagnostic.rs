// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact closed lifecycle diagnostics; output ownership supplies authentic run correlation.
use sugarsubstitute_application::startup::StartupPhase;
/// Identify the finite exit controls without exposing their payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    MainRetired,
    Release,
    ExitIntent,
    RestartToApply,
    ExitCompleted,
}
/// Distinguish preparation, native admission and actual reply consumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlStage {
    Prepared,
    Admitted,
    ReplyReceived,
    ReplyConsumed,
}
/// Name the owner supplying the unchanged control cutoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeadlineSource {
    ExitNotification,
    ShutdownTotal,
    AttemptCleanup,
    ControlInterval,
}
/// Describe the actual selected original cutoff without supplying authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Prepared {
    pub(crate) original_ns: u64,
    pub(crate) source: DeadlineSource,
    pub(crate) phase: Option<StartupPhase>,
    pub(crate) attempt_present: bool,
    pub(crate) attempt_complete: bool,
    pub(crate) shutdown_budget_present: bool,
}
pub(crate) use crate::startup::budget_trace::BudgetStage;
/// Fixed closed metadata is inert; the authenticated output owner supplies its Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseTrace {
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
    pub(crate) fn emit(self) {
        match self {
            Self::Control { stage, control, request, deadline_qpc, prepared: Some(p) } => {
                tracing::info!(operation="child_close_control",stage=?stage,control=?control,request,deadline_qpc,
                    original_ns=p.original_ns,deadline_source=?p.source,phase=?p.phase,
                    attempt_present=p.attempt_present,attempt_complete=p.attempt_complete,shutdown_budget_present=p.shutdown_budget_present);
            }
            Self::Control { stage, control, request, deadline_qpc, prepared: None } => {
                tracing::info!(operation="child_close_control",stage=?stage,control=?control,request,deadline_qpc);
            }
            Self::Budget { stage, moments } => {
                crate::startup::budget_trace::BudgetTrace { stage, moments }.emit()
            }
        }
    }
}
