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
    lifecycle::{RunKey, UiChallengeKey, UiWatchdogActivity},
    startup::{LaunchId, ReadinessReceipt, StartupFailure},
};

pub use sugarsubstitute_contracts::startup::MainIdentity;

/// Borrow localized native caption names from the authoritative locale owner.
#[derive(Clone, Copy, Debug)]
pub struct MainCaptionLabels {
    /// Accessible minimize control name.
    pub minimize: &'static str,
    /// Accessible maximize control name.
    pub maximize: &'static str,
    /// Accessible restore control name.
    pub restore: &'static str,
    /// Accessible close control name.
    pub close: &'static str,
}

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
    lease: MemoryLease,
    metadata: MemoryClaim,
}

impl MainWindowRequest {
    /// Conservative initial ceiling for known native-observer storage, in bytes.
    pub const KNOWN_STORAGE_BYTES: u64 = 4 * 1024 * 1024;
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
        if lease.bytes() < Self::KNOWN_STORAGE_BYTES {
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
    /// Borrow retained byte credit for explicit bounded storage claims.
    /// Claiming bytes does not submit work or grant native privileges.
    pub fn storage_lease(&self) -> &MemoryLease {
        &self.lease
    }
    /// Return checked retained request-metadata capacity, including its fixed owner header.
    /// The value excludes opaque browser allocations and grants no admission.
    pub fn allocated_metadata_bytes(&self) -> u64 {
        self.metadata.bytes()
    }
}

/// Reject invalid main-host input without echoing paths or untrusted labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainRequestError {
    /// Initial logical dimensions are empty or exceed the project bound.
    Dimensions,
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
    /// Retire the exact current native main lifetime before admitting another attempt.
    RetireMain(MainIdentity),
    /// Fence creation and observe exact absence on the native thread. Outstanding
    /// application construction or an unsettled actor prevents a positive observation.
    ConfirmMainAbsent(MainIdentity),
    /// Close admission and begin the composing owner's actual shutdown.
    Close,
}

/// Transfer native facts to the authoritative application StartupSession.
pub enum DesktopStartupObservation {
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
    /// Complete native main/controller/callback retirement for the exact prior attempt.
    /// A failure observation or a requested COM close does not establish this fact.
    MainRetired(MainIdentity),
    /// The native owner fenced this current attempt and observed no matching actor,
    /// constructor, callback or handle custody. This is distinct from retiring a created actor.
    MainAbsent(MainIdentity),
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
