// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate native presentation through application-owned startup decisions.

use crate::{DesktopReceipt, NativeMaterial, WindowContent};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    time::Instant,
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, ProfileId},
    execution::{MemoryClaim, MemoryClaimError, MemoryLease},
    lifecycle::{LeaseKey, RunKey, UiChallengeKey, UiWatchdogActivity},
    startup::{LaunchId, ReadinessReceipt, StartupFailure},
};

use crate::{MainBackendState, MainCaptionLabels, MainRecoveryLabels, WorkflowActivity};
pub use sugarsubstitute_contracts::startup::MainIdentity;

/// Configure the initial main client rectangle in logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct MainWindowSettings {
    /// Positive initial width, at most 8192 logical pixels.
    pub logical_width: u32,
    /// Positive initial height, at most 8192 logical pixels.
    pub logical_height: u32,
    /// Native theme and fallback background selection.
    pub dark: bool,
    /// Localized native control names; web-owned text remains untouched.
    pub labels: MainCaptionLabels,
    /// Canonical localized native backend retry action/status.
    pub recovery: MainRecoveryLabels,
}

/// Retain version observations supplied by the verified served-source owner.
/// Inspected development source versions are not automatically runtime evidence.
#[derive(Debug)]
pub struct ServedFrontendVersions {
    frontend: String,
    sugarcubes: String,
}

impl ServedFrontendVersions {
    /// Bound owned labels before inserting them into a native probe result.
    /// The authoritative readiness parser also validates their version grammar.
    ///
    /// # Errors
    /// Rejects empty, NUL-bearing or greater-than-64-byte labels or capacities.
    pub fn new(frontend: String, sugarcubes: String) -> Result<Self, MainRequestError> {
        if [&frontend, &sugarcubes].iter().any(|label| {
            label.is_empty() || label.len() > 64 || label.capacity() > 64 || label.contains('\0')
        }) {
            return Err(MainRequestError::VersionLabel);
        }
        Ok(Self { frontend, sugarcubes })
    }

    /// Borrow the observed served ComfyUI frontend version.
    pub fn frontend(&self) -> &str {
        &self.frontend
    }
    /// Borrow the observed installed SugarCubes version.
    pub fn sugarcubes(&self) -> &str {
        &self.sugarcubes
    }
}

/// Transfer one admitted main-host request without transferring backend authority.
///
/// The root keeps its actual M1 profile lease until this native lifetime retires.
/// The byte lease covers explicitly claimed authored storage; opaque Wry/browser
/// allocations have separate measurement and are not covered by this constant.
pub struct MainWindowRequest {
    identity: MainIdentity,
    origin: LoopbackOrigin,
    profile: ProfileId,
    profile_directory: PathBuf,
    versions: ServedFrontendVersions,
    settings: MainWindowSettings,
    session_bootstrap: Option<crate::SessionBootstrap>,
    session_storage: Option<MemoryClaim>,
    initial_placement: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    placement_storage: Option<MemoryClaim>,
    lease: MemoryLease,
    metadata: MemoryClaim,
}

impl MainWindowRequest {
    /// Conservative initial ceiling for known native-observer storage, in bytes.
    pub const KNOWN_STORAGE_BYTES: u64 = 4 * 1024 * 1024;
    /// In-flight bulk panel transfer partition, in bytes: one 96 MiB message per direction.
    pub const BULK_TRANSFER_BYTES: u64 = 192 * 1024 * 1024;
    /// Independent eight-message session ingress/DTO partition, in bytes.
    pub const SESSION_STORAGE_BYTES: u64 = 24 * 1024 * 1024;
    /// Complete reservation a main lifetime requires: observer storage plus bulk transfers.
    pub const RESERVATION_BYTES: u64 =
        Self::KNOWN_STORAGE_BYTES + Self::BULK_TRANSFER_BYTES + Self::SESSION_STORAGE_BYTES;
    /// Maximum retained profile-path allocation, in platform encoded bytes.
    pub const PROFILE_PATH_BYTES: usize = 128 * 1024;

    /// Consume bounded adapter data and an already admitted reservation.
    /// This constructor performs no filesystem, browser or native operation.
    ///
    /// # Errors
    /// Rejects invalid initial geometry, caption labels, profile path syntax or
    /// insufficient reserved credit. Absolute path data does not prove trust or
    /// profile admission. Supplied metadata is already owned by the admitted caller;
    /// this constructor checks and claims actual retained capacities before transfer.
    /// Native owners claim new capacities before allocation and separately bound
    /// Windows UTF-16 conversion to 32767 units before making a native path copy.
    pub fn new(
        identity: MainIdentity,
        origin: LoopbackOrigin,
        profile: ProfileId,
        profile_directory: PathBuf,
        versions: ServedFrontendVersions,
        settings: MainWindowSettings,
        lease: MemoryLease,
    ) -> Result<Self, MainRequestError> {
        if settings.logical_width == 0
            || settings.logical_width > 8192
            || settings.logical_height == 0
            || settings.logical_height > 8192
        {
            return Err(MainRequestError::Dimensions);
        }
        if [
            settings.labels.minimize,
            settings.labels.maximize,
            settings.labels.restore,
            settings.labels.close,
            settings.recovery.retry,
            settings.recovery.retry_ready,
        ]
        .iter()
        .any(|label| label.is_empty() || label.len() > 128 || label.contains('\0'))
        {
            return Err(MainRequestError::CaptionLabel);
        }
        if !profile_directory.is_absolute()
            || profile_directory.as_os_str().len() > Self::PROFILE_PATH_BYTES
            || profile_directory.capacity() > Self::PROFILE_PATH_BYTES
            || profile_directory.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err(MainRequestError::ProfilePath);
        }
        if lease.bytes() < Self::RESERVATION_BYTES {
            return Err(MainRequestError::InsufficientMemory);
        }
        if profile.allocated_bytes() > 64 {
            return Err(MainRequestError::ProfileCapacity);
        }
        let metadata_bytes = profile_directory.capacity()
            + profile.allocated_bytes()
            + versions.frontend.capacity()
            + versions.sugarcubes.capacity()
            + size_of::<Self>();
        let metadata =
            lease.claim(metadata_bytes as u64).map_err(MainRequestError::MetadataBudget)?;
        Ok(Self {
            identity,
            origin,
            profile,
            profile_directory,
            versions,
            settings,
            session_bootstrap: None,
            session_storage: None,
            initial_placement: None,
            placement_storage: None,
            lease,
            metadata,
        })
    }

    /// Return immutable current application identity.
    pub fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Return the admitted HTTP loopback origin.
    pub fn origin(&self) -> LoopbackOrigin {
        self.origin
    }
    /// Borrow the profile namespace; this is not the root's actual profile lease.
    pub fn profile(&self) -> &ProfileId {
        &self.profile
    }
    /// Borrow the root-selected profile path without granting filesystem authority.
    pub fn profile_directory(&self) -> &Path {
        &self.profile_directory
    }
    /// Borrow verified served-version observations.
    pub fn versions(&self) -> &ServedFrontendVersions {
        &self.versions
    }
    /// Return initial logical geometry and native labels.
    pub fn settings(&self) -> MainWindowSettings {
        self.settings
    }
    /// Retain admitted normal outer placement before the native window is constructed.
    /// This supplies data only; the desktop validates current monitors on its creating thread.
    ///
    /// # Errors
    /// Rejects malformed placement, oversized retained monitor allocation or insufficient credit.
    pub fn set_initial_placement(
        &mut self,
        placement: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    ) -> Result<(), MainRequestError> {
        let storage = if let Some(value) = &placement {
            sugarsubstitute_contracts::desktop_session::validate_placement(value)
                .map_err(|_| MainRequestError::Placement)?;
            if value.monitor.capacity() > 1024 {
                return Err(MainRequestError::Placement);
            }
            Some(
                self.lease
                    .claim(value.monitor.capacity() as u64)
                    .map_err(MainRequestError::MetadataBudget)?,
            )
        } else {
            None
        };
        self.initial_placement = placement;
        self.placement_storage = storage;
        Ok(())
    }
    /// Borrow saved normal outer placement without granting access to native monitors or handles.
    pub fn initial_placement(
        &self,
    ) -> Option<&sugarsubstitute_contracts::desktop_session::wire::SessionPlacement> {
        self.initial_placement.as_ref()
    }
    /// Install host-selected session identity before any browser construction/navigation.
    /// The native channel compares every page message against this immutable bootstrap.
    /// # Errors
    /// Refuses foreign profile/origin or metadata that the original reservation cannot fund.
    pub fn set_session_bootstrap(
        &mut self,
        bootstrap: Option<crate::SessionBootstrap>,
    ) -> Result<(), MainRequestError> {
        if bootstrap.as_ref().is_some_and(|value| {
            value.origin() != self.origin || value.scope().profile != self.profile.as_str()
        }) {
            return Err(MainRequestError::SessionBootstrap);
        }
        let storage = bootstrap
            .as_ref()
            .map(|value| {
                self.lease
                    .claim((value.allocated_bytes() + size_of::<crate::SessionBootstrap>()) as u64)
            })
            .transpose()
            .map_err(MainRequestError::MetadataBudget)?;
        self.session_bootstrap = bootstrap;
        self.session_storage = storage;
        Ok(())
    }
    /// Borrow independently selected session identity; absence disables the native session lane.
    pub fn session_bootstrap(&self) -> Option<&crate::SessionBootstrap> {
        self.session_bootstrap.as_ref()
    }
    /// Borrow retained byte credit for explicit bounded storage claims.
    /// Claiming bytes does not submit work or grant native privileges.
    pub fn storage_lease(&self) -> &MemoryLease {
        &self.lease
    }
    /// Return checked retained request-metadata capacity, including its fixed owner header.
    /// The value excludes opaque browser allocations and grants no admission.
    pub fn allocated_metadata_bytes(&self) -> u64 {
        self.metadata.bytes()
            + self.session_storage.as_ref().map_or(0, MemoryClaim::bytes)
            + self.placement_storage.as_ref().map_or(0, MemoryClaim::bytes)
    }
}

/// Reject invalid main-host input without echoing paths or untrusted labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainRequestError {
    /// Initial logical dimensions are empty or exceed the project bound.
    Dimensions,
    /// Saved native placement or retained monitor storage is invalid.
    Placement,
    /// A native accessible label is empty, oversized or NUL-bearing.
    CaptionLabel,
    /// A version observation exceeds its owned-data bound.
    VersionLabel,
    /// The profile path is relative, oversized or contains an embedded NUL.
    ProfilePath,
    /// The supplied profile namespace retains more than 64 allocation bytes.
    ProfileCapacity,
    /// The admitted reservation cannot cover known observer storage.
    InsufficientMemory,
    /// Actual retained metadata cannot fit within the shared existing credit.
    MetadataBudget(MemoryClaimError),
    /// Session bootstrap differs from the actual requested profile or origin.
    SessionBootstrap,
}
impl fmt::Display for MainRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid main-host request: {self:?}")
    }
}
impl Error for MainRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MetadataBudget(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Carry the actual initiating reason through the existing Main retirement operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainRetirementReason {
    /// Retire the current failed or cancelled attempt without a timeout diagnosis.
    AttemptCleanup,
    /// The authoritative original frontend cutoff expired before readiness acceptance.
    FrontendTimeout,
    /// Replace an already revealed frontend while preserving its backend lease.
    FrontendReload,
}

/// Execute at most one explicit application command per bounded native drain.
pub enum DesktopStartupCommand {
    /// Process an authenticated matching challenge on the actual existing UI event loop.
    /// Closing retires queued challenges without granting a response.
    ProcessUiChallenge(UiChallengeKey),
    /// Create a covered, actual-size main host after successful splash submission.
    PrepareMain(Box<MainWindowRequest>),
    /// Advance only the exact paused environment after current-generation off-UI profile proof.
    AuthorizeEnvironment(MainIdentity),
    /// Permit frontend navigation only after exact-current retained browser-family proof.
    /// Environment/profile authorization alone does not admit this second fence.
    AuthorizeBrowser(MainIdentity),
    /// Request one read-only probe of the current native top document.
    ProbeFrontend(MainIdentity),
    /// Retire splash presentation while preserving the ready connection/main host.
    RetireSplash(LaunchId),
    /// Reveal an already usable main host after actual splash retirement.
    RevealMain(MainIdentity),
    /// Sample final placement on the creating thread while the exact native peer remains alive.
    /// The application retains this peer until its bounded save terminates, then requests close.
    CaptureMainPlacement {
        /// Exact current native lifetime; foreign and retired identities cannot capture.
        identity: MainIdentity,
        /// Positive application request identity, echoed in the terminal observation.
        capture_id: std::num::NonZeroU64,
    },
    /// Deliver a bounded host-direction panel message to the current top document.
    SendPanel(crate::PanelCommand),
    /// Deliver one bounded workspace operation through its independent session lane.
    SendSession(crate::SessionCommand),
    /// Complete an exact retained-media deferral on its creating native UI thread.
    SendSessionMedia(crate::SessionMediaReply),
    /// Deliver the latest memory allowance to the current top document; newer ones replace it.
    SendMemory(crate::MemoryCommand),
    /// Deliver an update state or answer to the current top document; a newer state replaces
    /// an unsent older one, and answers are never replaced.
    SendUpdates(crate::UpdatesCommand),
    /// Report the current browser environment's processes as [`DesktopStartupObservation::BrowserProcesses`].
    MeasureBrowser(MainIdentity),
    /// Ask the current browser to trim its memory (`low`) or return to normal use.
    SetBrowserMemoryTarget {
        /// Exact current native document identity.
        identity: MainIdentity,
        /// Whether the browser should reduce its memory use.
        low: bool,
    },
    /// Deliver a correlated native canvas protocol reply to the current top document.
    SendCanvasNative(crate::CanvasNativeReply),
    /// Deliver a native canvas notification without ordinary request credit; a newer one of
    /// the same kind replaces an undelivered one, and stale navigations drop it.
    SendCanvasNotification(crate::CanvasNativeNotification),
    /// Deliver a correlated canvas reply to the same current top document.
    SendSurface(crate::SurfaceReply),
    /// Deliver a native full presentation snapshot without ordinary request credit.
    SendSurfacePresentation(crate::SurfacePresentation),
    /// Complete consumed control or cancellation ingress while retaining its original allocation.
    CompletePanelIngress(crate::PanelIngressReceipt),
    /// Capture one current revealed renderer for an explicitly admitted native qualification.
    /// The inert cutoff grants no authentication or file authority and is never renewed.
    #[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
    CaptureMainPreview {
        /// Exact current authenticated application run, launch and navigation.
        identity: MainIdentity,
        /// Original finite campaign cutoff in absolute QPC ticks and ticks per second.
        source_deadline: sugarsubstitute_contracts::execution::CounterDeadline,
        /// Optional fixed test-only download campaign using an independently inspected PNG output.
        downloads: Option<crate::MainDownloadFixture>,
    },
    /// Present one current Main's failed backend lease as inert recovery data.
    /// Recovered clears only that exact retained outage; Parent validates actual replacement authority.
    PresentBackendOutage {
        /// Exact existing Main run, launch and navigation; no window/view recreation.
        identity: MainIdentity,
        /// Original failed backend interest; data does not authorize retry/replacement.
        failed: LeaseKey,
        /// Current composing-owner presentation, independent of frontend readiness.
        state: MainBackendState,
    },
    /// Replace a rendered outage only through the composing owner's exact retained handoff lineage.
    /// This handles successor failure before positive current-backend output, without false recovery.
    ReplaceBackendOutage {
        /// Same existing native Main; Root independently validates current backend authority.
        identity: MainIdentity,
        /// Exact currently rendered failed interest.
        previous: LeaseKey,
        /// Distinct successor's actual failed interest, same run/connection/owner.
        failed: LeaseKey,
        /// Recovery presentation for the successor failure; Recovered is invalid here.
        state: MainBackendState,
    },
    /// Retire the exact current native main lifetime before admitting another attempt.
    RetireMain {
        /// Exact current actor; a rendered identity grants no native authority.
        identity: MainIdentity,
        /// The authoritative initiating attempt failure; diagnosis is passive.
        reason: MainRetirementReason,
    },
    /// Fence creation and observe exact absence on the native thread. Outstanding
    /// application construction or an unsettled actor prevents a positive observation.
    ConfirmMainAbsent(MainIdentity),
    /// Close admission and begin the composing owner's actual shutdown.
    Close,
}

/// What a closing native session does with one queued command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClosingAdmission {
    /// Close-owned work still runs.
    Run,
    /// Traffic addressed to the leaving page is dropped: nothing remains to receive it.
    Drop,
    /// Any other command after close began is an owner error.
    Refuse,
}
impl DesktopStartupCommand {
    /// Classify this command once the native session began closing. A page message produced by
    /// work that finished as the window closed, such as a workspace save's outcome, is dropped.
    pub(crate) const fn while_closing(&self) -> ClosingAdmission {
        match self {
            Self::Close
            | Self::RetireMain { .. }
            | Self::ConfirmMainAbsent(_)
            | Self::ProcessUiChallenge(_) => ClosingAdmission::Run,
            Self::SendSurfacePresentation(_)
            | Self::SendCanvasNotification(_)
            | Self::SendSession(_)
            | Self::SendSessionMedia(_) => ClosingAdmission::Drop,
            _ => ClosingAdmission::Refuse,
        }
    }
}

/// Native cause of a current revealed frontend replacement. This grants no native privilege.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainReloadReason {
    /// An actual same-origin top navigation requested replacement, including browser reload keys.
    Navigation,
    /// An actual WebView process failure; the platform adapter retains the numeric kind separately.
    ProcessFailed,
}

/// Transfer native facts to the authoritative application StartupSession.
pub enum DesktopStartupObservation {
    /// Actual native normal bounds; changes coalesce independently of session publication.
    MainPlacementObserved(crate::MainPlacementObservation),
    /// Placement sampling failed without retiring the live native peer or replacing durable state.
    MainPlacementCaptureFailed {
        /// Exact lifetime addressed by the original application request.
        identity: MainIdentity,
        /// Original positive application capture request.
        capture_id: Option<std::num::NonZeroU64>,
        /// Preserved getter/admission cause; this is not a successful empty capture.
        cause: crate::DesktopError,
    },
    /// Actual native eligibility from owned windows and acknowledged native suspension.
    NativeActivity {
        /// Exact retained desktop run; page state supplies no native evidence.
        run: RunKey,
        /// Positive monotonic native revision; never reused within this lifetime.
        revision: std::num::NonZeroU64,
        /// Actual visible active, inactive or acknowledged suspended native state.
        activity: UiWatchdogActivity,
    },
    /// The actual UI event loop dispatched the exact admitted challenge.
    UiChallengeProcessed(UiChallengeKey),
    /// Main receivers/controllers/windows and splash controls/presenter/window have retired.
    /// Content may still communicate final completion; this does not prove executor settlement.
    NativeRetired(RunKey),
    /// The splash's native pixels and controls were successfully submitted.
    SplashPresented(LaunchId),
    /// Effective native folder data, awaiting authoritative file-ID/live-profile-lease validation.
    EnvironmentPrepared {
        /// Exact current main construction identity.
        identity: MainIdentity,
        /// Immutable observed UTF-16 and its retained known-storage admission.
        facts: crate::main_environment::MainEnvironmentFacts,
    },
    /// Actual native browser process observation while the constructed view remains at about:blank.
    /// This numeric observation is data; the off-UI family owner supplies its retained kernel proof.
    ControllerPrepared {
        /// Exact current main construction identity.
        identity: MainIdentity,
        /// Positive native BrowserProcessId observed from the actual current WebView instance.
        browser_process_id: u32,
    },
    /// Native navigation admission succeeded for this current identity.
    NavigationStarted(MainIdentity),
    /// Bounded, origin/context/barrier-checked frontend evidence.
    FrontendReceipt {
        /// Exact current native document identity.
        identity: MainIdentity,
        /// Existing validated readiness wire contract.
        receipt: ReadinessReceipt,
    },
    /// Native splash resources actually retired without cancelling the connection.
    SplashRetired(LaunchId),
    /// Main cover retirement and native show/control/focus operations succeeded.
    MainRevealed(MainIdentity),
    /// A bounded panel message from the current native top-document context.
    PanelMessage(crate::PanelObservation),
    /// A bounded workspace message from this current independently bootstrapped native page.
    SessionMessage(crate::SessionObservation),
    /// A retained-media GET awaiting execution-owned identity validation and encoded bytes.
    SessionMediaRequested(crate::SessionMediaObservation),
    /// The native media deadline/page lifetime expired; the application cancels and observes work.
    SessionMediaCancelled {
        /// Actual current main lifetime that owned the request.
        identity: MainIdentity,
        /// Original native per-lifetime request identity.
        request_id: std::num::NonZeroU64,
    },
    /// A bounded memory report from the current native top-document context.
    MemoryMessage(crate::MemoryObservation),
    /// A bounded update request from the current native top-document context.
    UpdatesMessage(crate::UpdatesObservation),
    /// The processes the current browser environment runs, for the memory budget's footprint.
    BrowserProcesses {
        /// Exact current native document identity.
        identity: MainIdentity,
        /// Operating-system process identities; numeric data, not process custody.
        processes: Vec<u32>,
    },
    /// A bounded canvas message from the current native top-document context.
    SurfaceMessage(crate::SurfaceObservation),
    /// A bounded native canvas protocol request from the current native top-document context.
    CanvasNativeMessage(crate::CanvasNativeObservation),
    /// A pointer or wheel gesture the main window routed to the native canvas input region.
    /// It is delivered after the UI-thread message returned and never waits for canvas work.
    #[cfg(feature = "native-composition")]
    CanvasInput(crate::canvas::MainCanvasInput),
    /// The refresh rate of the display now showing the composed main window's canvas.
    #[cfg(feature = "native-composition")]
    CanvasDisplay(crate::canvas::MainCanvasDisplay),
    /// A native callback requested replacement of the exact already revealed document.
    MainReloadRequested {
        /// Full current run, launch and navigation; stale callbacks cannot select another Main.
        identity: MainIdentity,
        /// Native cause, independent of page messages or frontend readiness evidence.
        reason: MainReloadReason,
    },
    /// Actual current renderer PNG after its native completion and document/callback checks.
    #[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
    MainPreviewCaptured(crate::CapturedMainPreview),
    /// Preserve optional native qualification failure independently of product startup health.
    #[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
    MainPreviewFailed {
        /// The current native main identity for the failed qualification attempt.
        identity: MainIdentity,
        /// Actual adapter/native cause; an error does not establish callback retirement.
        cause: crate::DesktopError,
    },
    /// Complete native main/controller/callback retirement for the exact prior attempt.
    /// A failure observation or a requested COM close does not establish this fact.
    MainRetired(MainIdentity),
    /// The native owner fenced this current attempt and observed no matching actor,
    /// constructor, callback or handle custody. This is distinct from retiring a created actor.
    MainAbsent(MainIdentity),
    /// One native retry click consumed from the exact currently enabled outage control.
    /// This pair is observational input; Parent independently authenticates and admits retry.
    BackendRetryRequested {
        /// Exact retained Main; the document/navigation remain unchanged.
        identity: MainIdentity,
        /// Failed lease captured by that control's current presentation revision.
        failed: LeaseKey,
    },
    /// Native close intent; the application chooses mode-appropriate cleanup.
    CloseRequested,
    /// Preserve a native cause independently of retained partial resource ownership.
    Failed {
        /// Current identity, absent if main preparation never began.
        identity: Option<MainIdentity>,
        /// Original UI-affine adapter cause; partial native resources remain with desktop.
        failure: StartupFailure,
    },
}

/// Return actual pre-surface preparation progress on the retained native loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupPreparationStep {
    /// Authentication, capture attachment or their bounded completion disposal remains pending.
    Waiting,
    /// The role's actual required bootstrap authentication and capture attachment completed.
    Ready,
    /// Admission is closed; retain execution/transport/capture ownership until actual settlement.
    Closing,
}

/// Extend the accepted splash content boundary with actual startup composition.
///
/// Every method runs on the one native event-loop thread. Implementations reuse
/// StartupSession; they do not construct another startup policy. Calls must be
/// bounded/nonblocking. Native error observations never claim work stopped.
pub trait StartupWindowContent: WindowContent {
    /// Retain the exact main before its composition lifetime begins.
    /// A false result defers native composition construction while an earlier lifetime settles.
    ///
    /// # Errors
    /// Rejects foreign or overlapping native main lifetimes.
    #[cfg(all(windows, feature = "native-composition"))]
    fn main_composition_started(
        &mut self,
        _identity: MainIdentity,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(true)
    }

    /// Transfer one compositor wait only when application-owned admission can retain it.
    /// A false result leaves the original input in the desktop owner.
    ///
    /// # Errors
    /// Preserves invalid identity, deadline and execution admission failures.
    #[cfg(all(windows, feature = "native-composition"))]
    fn main_composition_wait(
        &mut self,
        _identity: MainIdentity,
        _input: &mut Option<crate::canvas::CompositionWait>,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(false)
    }

    /// Select upload, qualified retention, or withdrawal without granting window authority.
    /// Retention names the acknowledged frame and cannot acknowledge a pending replacement.
    #[cfg(feature = "native-composition")]
    fn main_canvas_presentation(&self) -> crate::canvas::MainCanvasPresentation<'_> {
        crate::canvas::MainCanvasPresentation::Hide
    }

    /// Publish where routed native gestures may reach the shown final image, or `None`.
    /// Called on every main-loop turn after presentation; the desktop withdraws the region when
    /// its identity, allocation or acknowledged placement no longer matches native state.
    #[cfg(feature = "native-composition")]
    fn main_canvas_input_region(&self) -> Option<crate::canvas::MainCanvasInputRegion<'_>> {
        None
    }

    /// Whether the main canvas has timed work due within a few display frames, so the event
    /// loop needs a fine system timer period to meet display-rate deadlines.
    fn main_canvas_animating(&self) -> bool {
        false
    }

    /// Close because Windows is ending the session, keeping everything authored. A `critical`
    /// session end does not wait for any answer, so the content saves on a short budget.
    /// Content without a close save simply closes.
    ///
    /// # Errors
    /// Preserves the close transaction's first measurement or cleanup failure.
    fn request_session_close(&mut self, critical: bool) -> Result<(), Box<dyn Error>> {
        let _ = critical;
        self.request_shutdown()
    }

    /// Whether the close save has settled, so a session end may proceed. Content without a
    /// close save holds nothing.
    fn close_save_settled(&self) -> bool {
        true
    }

    /// The localized reason Windows shows while a session end waits for the close save.
    fn session_end_reason(&self) -> &'static str {
        sugarsubstitute_contracts::localization::messages("en").closing.saving
    }

    /// Hand over a prepared drag-out of the shown output, at most once per routed request.
    /// Called on every main-loop turn after routed gestures; the desktop discards an output for
    /// another lifetime or one whose primary press already ended.
    #[cfg(feature = "native-composition")]
    fn take_main_canvas_drag_out(&mut self) -> Option<crate::canvas::MainCanvasDragOut> {
        None
    }

    /// Acknowledge a matching frame only after native upload and composition commit succeed.
    ///
    /// # Errors
    /// Rejects duplicate, stale or foreign frame acknowledgements.
    #[cfg(feature = "native-composition")]
    fn main_canvas_presented(
        &mut self,
        _target: sugarsubstitute_canvas::FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        Err("Main canvas acknowledgement has no owner".into())
    }

    /// Observe that the compositor processed the exact main window's canvas hide.
    /// This does not establish physical scanout.
    ///
    /// # Errors
    /// Preserves invalid current attachment or reply-admission failures.
    #[cfg(feature = "native-composition")]
    fn main_canvas_hidden(&mut self, _identity: MainIdentity) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Route at most `min(maximum, 32)` shared completion notices before native UI admission.
    ///
    /// Called after the single start attempt on the creating main thread, before Tao, STA entry,
    /// splash controls or a WebView. Ready requires actual current authentication and
    /// required capture attachment; it cannot be inferred from launch intent. Closing
    /// continues bounded terminal disposal. Implementations retain original deadlines.
    ///
    /// # Errors
    /// Preserve the original cause while the same main-thread pump continues owned shutdown.
    fn prepare_native_ui(
        &mut self,
        maximum: std::num::NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>>;

    /// Adopt the operating-system accent for the splash, main window and SugarCubes.
    ///
    /// Called on the native thread once before the splash initializes and again whenever the
    /// reported accent changes. `None` means the platform reports no accent; surfaces then use
    /// the product palette. The call must not wait for preparation or page delivery.
    ///
    /// # Errors
    /// Preserves application failures to retain or forward the new presentation.
    fn system_accent(
        &mut self,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    ) -> Result<(), Box<dyn Error>>;

    /// Return the immutable coordinator-bound run for this desktop's entire native lifetime.
    /// A retry may change launch/navigation identities while retaining this exact run.
    fn run_key(&self) -> RunKey;
    /// Return the current launch from the authoritative application startup session.
    /// Frame and navigation generations cannot substitute for this identity.
    fn launch_id(&self) -> LaunchId;
    /// Exact current identity from authoritative application lifecycle state, absent
    /// until that state exists. Render/presentation identity grants no native admission.
    fn main_identity(&self) -> Option<MainIdentity>;
    /// Observe retained original Main construction/input/jobs at the application owner.
    /// False cannot establish native absence; the desktop joins its own actor custody.
    fn main_construction_pending(&self) -> bool;

    /// Transfer one actual native observation into application startup policy.
    ///
    /// # Errors
    /// Preserve the original adapter/application cause; shutdown remains owned.
    fn observe(&mut self, observation: DesktopStartupObservation) -> Result<(), Box<dyn Error>>;
    /// Transfer one pending application command; the desktop drains at most 32.
    ///
    /// # Errors
    /// Command production failure retains its cause and closes new admission.
    fn take_command(&mut self) -> Result<Option<DesktopStartupCommand>, Box<dyn Error>>;
    /// Return the earliest actual application deadline, without renewing it.
    fn next_deadline(&self) -> Option<Instant>;

    /// The user chose to restart so a prepared update applies. The application runs its
    /// ordinary close, keeping everything authored, and relaunches once that close has an
    /// outcome; a close already under way keeps its own reason. Called on the event-loop thread
    /// between native callbacks; returns promptly. A host without an updater never offers it.
    ///
    /// # Errors
    /// Preserves the close transaction's failure; this default refuses because the host offers
    /// no update to apply.
    fn request_restart_to_apply(&mut self) -> Result<(), Box<dyn Error>> {
        Err("This host offers no update to apply".into())
    }

    // What the window shows about updates, the close save and last session's losses. Each is
    // read on the event-loop thread at most once per turn and must return promptly.

    /// The host's update status, or `None` for a host without an updater. The caption's update
    /// button is hidden unless an update is ready or the latest attempt failed.
    fn update_status(&self) -> Option<sugarsubstitute_contracts::update::status::UpdateStatus> {
        None
    }

    /// The close save's progress while the window stays up for it; `None` before close began.
    fn close_save_progress(
        &self,
    ) -> Option<sugarsubstitute_application::workspace_session::close_save::CloseSaveProgress> {
        None
    }

    /// Take, once, what the previous close did not keep. The window shows it as one line the
    /// user can dismiss; it is taken only once the main window can show it.
    fn take_unsaved_notice(
        &mut self,
    ) -> Option<sugarsubstitute_application::workspace_session::close_save::NotSavedLastTime> {
        None
    }

    /// Whether a workflow is running, so the restart offer can say it waits for it.
    fn workflow_activity(&self) -> WorkflowActivity {
        WorkflowActivity::Idle
    }

    /// Take the writer that exports the current update report to a file the user selects,
    /// once per opened report. `None` hides Export; Copy stays available.
    fn update_report_export(&mut self) -> Option<Box<dyn crate::report::surface::UpdateExport>> {
        None
    }

    /// The user's language for native caption lines and reports opened from the main window.
    fn locale_messages(&self) -> &'static sugarsubstitute_contracts::localization::Messages {
        sugarsubstitute_contracts::localization::messages("en")
    }
}

/// Return presentation evidence only after native and composing-owner teardown.
pub struct StartupDesktopReceipt {
    /// Actual splash submissions through its accepted presenter.
    pub splash: DesktopReceipt,
    /// Material actually accepted for the optional main window.
    pub main_material: Option<NativeMaterial>,
    /// Successful admitted current frontend navigations.
    pub navigations_started: u64,
}

#[cfg(test)]
#[path = "main_window_tests.rs"]
mod tests;
