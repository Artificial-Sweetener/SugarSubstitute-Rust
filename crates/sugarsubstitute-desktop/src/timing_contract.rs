// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define raw, bounded native callback and successful-submission evidence.

use super::{NativeStartupOrder, NativeStartupTiming};
use sugarsubstitute_contracts::presentation::FrameIdentity;

/// Identify the native counter that owns every tick in one receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTimingSource {
    /// Windows QueryPerformanceCounter with its measured ticks-per-second frequency.
    WindowsQpc,
}

/// Retain an inclusive begin/end observation in raw counter ticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTickInterval {
    /// Counter value immediately before the observed segment.
    pub begin_ticks: i64,
    /// Counter value immediately after the observed segment.
    pub end_ticks: i64,
}

/// Classify an authored Tao closure or installed Windows subclass callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCallbackKind {
    /// Complete authored parent subclass body, including ordinary default-procedure chaining.
    WindowsParent,
    /// Complete authored accessible child subclass body and its ordinary default chain.
    WindowsChild,
    /// Native window input, geometry or lifecycle event.
    WindowEvent,
    /// Bounded application terminal metadata/result draining.
    CompletionDrain,
    /// Native control activation, close or focus intent.
    NativeAction,
    /// Submission of retained immutable pixels and native controls.
    Redraw,
    /// Preparation cadence, closing drain and actual shutdown polling.
    LoopProgress,
    /// Other authored event-loop callbacks, including initialization and destruction events.
    Other,
}

/// Observe one complete authored event-loop callback without counting delegated drag as host work.
///
/// Parent/child subclass bodies and the Tao event-loop closure are separate sample kinds.
/// Nested authored callbacks, when delivered, produce their own raw observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCallbackTiming {
    /// One-based callback completion sequence; nested intervals can overlap.
    pub sequence: u64,
    /// The callback's typed event category.
    pub kind: NativeCallbackKind,
    /// Raw value at authored callback entry.
    pub begin_ticks: i64,
    /// Raw value after the complete authored callback and control-flow decision.
    pub end_ticks: i64,
    /// Exact parent SC_MOVE DefSubclassProc modal interval, excluded from authored ticks.
    /// Queuing movement remains authored work; nested callbacks have separate samples.
    pub delegated_drag: Option<NativeTickInterval>,
    /// Checked full callback ticks minus only the delegated drag-call interval.
    pub authored_ticks: u64,
}

/// Observe one successful native/control/source-audit submission with its actual frame identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSubmissionTiming {
    /// One-based successful submission sequence, including retained native exposures.
    pub sequence: u64,
    /// Exact immutable source identity submitted successfully.
    pub identity: FrameIdentity,
    /// True when identity equals the most recent actual successful submission.
    /// Such exposures are not new producer samples.
    pub retained: bool,
    /// Raw counter value before native presentation work begins.
    pub begin_ticks: i64,
    /// Raw value after native submission, controls and any enabled source audit succeed.
    pub end_ticks: i64,
}

/// Transfer opt-in timing observations after native teardown, without pixels or trace files.
///
/// Both buffers allocate at most 16,384 samples before callback processing starts.
/// Raw signed integer ticks cannot represent NaN or infinity. Source, frequency,
/// thread affinity, ordering and integer arithmetic are validated before recording.
/// Callback measurements cover authored Tao closure/drain and installed subclass bodies.
/// Raw intervals preserve nesting; analyze distributions by kind instead of summing overlaps.
/// Compositor latency is outside these measurements. Nonzero omissions mean incomplete evidence.
#[derive(Debug)]
pub struct NativeTimingReceipt {
    /// Exact immutable construction order owning the fixed startup slots.
    pub startup_order: NativeStartupOrder,
    /// Clock identity shared by every observation.
    pub source: NativeTimingSource,
    /// First native adapter counter observation, before window and content construction.
    pub begin_ticks: i64,
    /// Final observation after native and owned content teardown.
    pub end_ticks: i64,
    /// QueryPerformanceFrequency in counter ticks per second; always positive.
    pub frequency: u64,
    /// Creating native UI thread; every sample is validated against this identity.
    pub ui_thread_id: u32,
    /// At most 16,384 complete authored event-loop callback observations.
    pub callbacks: Vec<NativeCallbackTiming>,
    /// At most 16,384 successful native/control submissions, including retained exposures.
    pub submissions: Vec<NativeSubmissionTiming>,
    /// Seven fixed, ordered setup/request slots; failure may leave an observed prefix.
    /// Ticks share the receipt's validated source, frequency and UI thread identity.
    pub startup_milestones: [Option<NativeStartupTiming>; 7],
    /// All callback observations, including samples beyond the storage bound.
    pub callbacks_observed: u64,
    /// Callback observations omitted after the fixed buffer filled.
    pub callback_samples_omitted: u64,
    /// All successful submissions, including samples beyond the storage bound.
    pub submissions_observed: u64,
    /// Successful submissions omitted after the fixed buffer filled.
    pub submission_samples_omitted: u64,
}
