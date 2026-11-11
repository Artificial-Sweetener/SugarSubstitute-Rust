// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact development diagnostics for the authenticated child's bounded stderr decoder.
//! These records describe accepted effects; they grant no native or readiness authority.
use crate::MainIdentity;
use std::fmt;
mod native;
pub use native::{NativeFailure, NativePhase, native_failed, native_operation};

/// A closed observation of one creating-thread or registered preview transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    /// The creating thread dispatched environment authorization.
    EnvironmentDispatched,
    /// The creating thread dispatched browser authorization.
    BrowserDispatched,
    /// The actual Main owner accepted browser authorization.
    BrowserAuthorized,
    /// The native local navigation call succeeded.
    NavigationStarted,
    /// The current native navigation completed successfully.
    NavigationCompleted,
    /// A current positive frontend receipt passed its native barrier.
    FrontendReady,
    /// A current negative frontend receipt passed its native barrier.
    FrontendFailed,
    /// The creating thread dispatched reveal after splash retirement.
    RevealDispatched,
    /// The actual Main owner completed reveal.
    MainRevealed,
    /// The registered driver requested its one current Main preview.
    PreviewRequested,
    /// The creating thread dispatched the native capture request.
    PreviewDispatched,
    /// The native profile destination preceded the one fixed download script submission.
    DownloadsPrepared,
    /// Three production-admitted JSON/PNG downloads actually completed in the sealed Main.
    DownloadsCompleted,
    /// The current web document sent a valid canvas context request.
    CanvasContextObserved,
    /// The current panel did not admit the canvas context request.
    CanvasContextRejected,
    /// The request is waiting for the canvas session to finish initialization.
    CanvasContextWaitingSession,
    /// The request is waiting for a current authenticated backend.
    CanvasContextWaitingBackend,
    /// The correlated canvas context reply entered the native command queue.
    CanvasContextReplyQueued,
}
impl Progress {
    /// Decode only a known closed schema-one stage.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "EnvironmentDispatched" => Some(Self::EnvironmentDispatched),
            "BrowserDispatched" => Some(Self::BrowserDispatched),
            "BrowserAuthorized" => Some(Self::BrowserAuthorized),
            "NavigationStarted" => Some(Self::NavigationStarted),
            "NavigationCompleted" => Some(Self::NavigationCompleted),
            "FrontendReady" => Some(Self::FrontendReady),
            "FrontendFailed" => Some(Self::FrontendFailed),
            "RevealDispatched" => Some(Self::RevealDispatched),
            "MainRevealed" => Some(Self::MainRevealed),
            "PreviewRequested" => Some(Self::PreviewRequested),
            "PreviewDispatched" => Some(Self::PreviewDispatched),
            "DownloadsPrepared" => Some(Self::DownloadsPrepared),
            "DownloadsCompleted" => Some(Self::DownloadsCompleted),
            "CanvasContextObserved" => Some(Self::CanvasContextObserved),
            "CanvasContextRejected" => Some(Self::CanvasContextRejected),
            "CanvasContextWaitingSession" => Some(Self::CanvasContextWaitingSession),
            "CanvasContextWaitingBackend" => Some(Self::CanvasContextWaitingBackend),
            "CanvasContextReplyQueued" => Some(Self::CanvasContextReplyQueued),
            _ => None,
        }
    }
}
/// A closed cause category; original errors stay with their actual owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cause {
    /// The original controller state rejected the request.
    State,
    /// Actual identities did not match.
    Identity,
    /// The original campaign deadline expired.
    Deadline,
    /// Original funded storage was insufficient.
    Capacity,
    /// The execution owner rejected an operation.
    Execution,
    /// Original queue admission failed.
    Admission,
    /// The actual job completion did not correlate.
    JobIdentity,
    /// The original native capture failed.
    Native,
    /// The actual artifact writer failed.
    Persistence,
    /// A genuine job terminal carried no successful artifact.
    Terminal,
    /// Original capture storage declaration failed.
    Declaration,
    /// Genuine registered source arming failed.
    Arming,
}
impl Cause {
    /// Decode only a known category, without retaining error text or credentials.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "State" => Some(Self::State),
            "Identity" => Some(Self::Identity),
            "Deadline" => Some(Self::Deadline),
            "Capacity" => Some(Self::Capacity),
            "Execution" => Some(Self::Execution),
            "Admission" => Some(Self::Admission),
            "JobIdentity" => Some(Self::JobIdentity),
            "Native" => Some(Self::Native),
            "Persistence" => Some(Self::Persistence),
            "Terminal" => Some(Self::Terminal),
            "Declaration" => Some(Self::Declaration),
            "Arming" => Some(Self::Arming),
            _ => None,
        }
    }
}
/// Fixed passive facts from an activated artifact with checked native handle retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PngProof {
    /// Original PNG length, in bytes, at most two MiB.
    pub bytes: u64,
    /// SHA-256 calculated by the actual central persister.
    pub digest: [u8; 32],
    /// Native WebView navigation ID of the captured current document.
    pub native_navigation: u64,
    /// Physical browser viewport width, in pixels.
    pub width: u32,
    /// Physical browser viewport height, in pixels.
    pub height: u32,
    /// Actual window dots per inch.
    pub dpi: u32,
}
struct Digest<'a>(&'a [u8; 32]);
impl fmt::Display for Digest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
/// Emit one actual accepted transition without allocating or serializing a RunKey.
pub fn progress(stage: Progress, identity: MainIdentity) {
    tracing::info!(operation="child_native_qualification", schema=1, stage=?stage,
        main=true, launch=identity.launch.get(), navigation=identity.navigation.get());
}
/// Emit one original qualification failure; absent Main remains explicitly absent.
pub fn failure(identity: Option<MainIdentity>, cause: Cause) {
    tracing::warn!(operation="child_native_qualification", schema=1, stage="PreviewFailed",
        main=identity.is_some(), launch=identity.map_or(0, |id| id.launch.get()),
        navigation=identity.map_or(0, |id| id.navigation.get()), cause=?cause);
}
/// Emit only after the actual sealed worker's activated artifact has checked native retirement.
pub fn persisted(identity: MainIdentity, proof: PngProof) {
    tracing::info!(operation="child_native_qualification", schema=1, stage="PreviewPersisted",
        main=true, launch=identity.launch.get(), navigation=identity.navigation.get(),
        png_bytes=proof.bytes, png_sha256=%Digest(&proof.digest), native_navigation=proof.native_navigation,
        viewport_width=proof.width, viewport_height=proof.height, dpi=proof.dpi);
}
/// Emit only after actual desktop startup returns its successful shutdown receipt.
pub fn finished() {
    tracing::info!(
        operation = "child_native_qualification",
        schema = 1,
        stage = "NativeFinished",
        main = false,
        launch = 0,
        navigation = 0
    );
}
