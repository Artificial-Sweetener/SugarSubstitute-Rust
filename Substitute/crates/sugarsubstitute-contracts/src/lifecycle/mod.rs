// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define in-process supervision facts without native handles or authentication claims.
//! IPC, native containment and process registry adapters remain separate boundaries.
mod backend_binding;
mod desktop_exit;
mod error;
mod identity;
mod resource;
mod watchdog;
pub use backend_binding::BackendBindingDigest;
pub use desktop_exit::{CompletionIntent, DesktopExitKind, DesktopObservation, ShutdownBudget};
pub use error::{LifecycleFailure, LifecycleFailureCode, LifecycleRejection, RejectedObservation};
pub use identity::{
    ConnectionId, CoordinatorEpoch, CoordinatorKey, IdentityError, InstanceId, LeaseId, LeaseKey,
    NamespaceId, ProcessId, ProcessKey, RequestId, ResourceGeneration, ResourceId, ResourceKey,
    RunGeneration, RunKey, ShutdownId,
};
pub use resource::{
    BackendTarget, ClientBinding, ClientRegistration, GrantedResourceInvalidation,
    GrantedResourceReplacement, RequestOutcome, ResourceCommand, ResourceEffect,
    ResourceObservation, ResourceReply, ResourceRequest, ResourceTransition, RunEnrollment,
};
pub use watchdog::{
    UI_CHALLENGE_CADENCE, UI_SUSPICION_THRESHOLD, UiChallengeId, UiChallengeKey,
    UiWatchdogActivity, UiWatchdogEffect, UiWatchdogObservation,
};
