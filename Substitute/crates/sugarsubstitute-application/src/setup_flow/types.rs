// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Typed native installer intents and observed outcomes. These are not persisted formats.

/// Current installation page, independent of native widget identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// Language selection.
    Language,
    /// Installation folder selection and inspection.
    Location,
    /// Managed or existing ComfyUI selection.
    Runtime,
    /// Upstream environment options or existing runtime paths.
    Configuration,
    /// Model/output folder selection; no models are acquired here.
    Folders,
    /// Update/repair an inspected native installation.
    Maintenance,
    /// One installation operation and its output.
    Progress,
    /// Completed installation with an explicit Open action.
    Complete,
}

/// Ownership choice for the ComfyUI runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Provision through the upstream installer backend.
    Managed,
    /// Validate and attach to the user's existing environment.
    Attached,
}

/// Editable path identity; file operations remain with the installation adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathField {
    /// Native application installation root.
    Installation,
    /// ComfyUI installation root.
    Comfy,
    /// Optional existing Python interpreter.
    Python,
    /// Preserved model folder.
    Models,
    /// Preserved output folder.
    Outputs,
}

/// Upstream option dependency order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionField {
    /// Upstream release channel.
    Release,
    /// Version within the selected release channel.
    ComfyVersion,
    /// Compatible environment/hardware package.
    Variant,
}
impl OptionField {
    /// Stable ephemeral array slot for the three dependent option lists.
    pub(super) fn index(self) -> usize {
        match self {
            Self::Release => 0,
            Self::ComfyVersion => 1,
            Self::Variant => 2,
        }
    }
}

/// User-entered setup choices. Storage/provisioning validates every submitted proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    /// Supported locale identifier.
    pub locale: &'static str,
    /// Runtime ownership intent.
    pub mode: RuntimeMode,
    /// Native application folder.
    pub install_root: String,
    /// ComfyUI folder.
    pub comfy_root: String,
    /// Optional existing interpreter path.
    pub python: String,
    /// Preserved model folder.
    pub models: String,
    /// Preserved output folder.
    pub outputs: String,
    /// Upstream-issued environment selection; never reconstructed from display labels.
    pub selection_token: Option<String>,
}

/// One upstream choice, with its original label and opaque identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// Upstream option identity.
    pub value: String,
    /// Original upstream display label.
    pub label: String,
    /// Original upstream explanatory text.
    pub description: String,
    /// Select this recommendation when loading the list.
    pub recommended: bool,
    /// Opaque provisioning selection supplied for an environment variant.
    pub selection_token: Option<String>,
}

/// Presentation-only facts from a successful native installation inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingInstallation {
    /// Inspected installation root.
    pub install_root: String,
    /// Preserved ComfyUI root.
    pub comfy_root: String,
    /// Display executable-repair guidance.
    pub executable_missing: bool,
    /// Display launch-entry repair guidance, which takes precedence over executable guidance.
    pub entry_missing: bool,
}

/// Inspected folder defaults, with optional native maintenance facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Default ComfyUI root for a fresh installation.
    pub comfy_root: String,
    /// Default preserved model folder.
    pub models: String,
    /// Default preserved output folder.
    pub outputs: String,
    /// Native installation requiring the maintenance route.
    pub existing: Option<ExistingInstallation>,
}

/// Surface-local request identity, distinct from execution jobs and worker protocol identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(pub(super) u64);
impl RequestId {
    /// Nonzero monotonically increasing value for host correlation.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Application operation admitted by a native user intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Adopt a supported locale for labels and subsequent backend progress.
    Locale(&'static str),
    /// Inspect an installation directory and return defaults or maintenance facts.
    Inspect(String),
    /// Discover the next upstream option list using current selections.
    Options {
        /// List to discover.
        field: OptionField,
        /// Selected release identity, when present.
        release: Option<String>,
        /// Selected version identity, when present.
        version: Option<String>,
        /// Supported locale.
        locale: &'static str,
        /// Upstream download-mirror preference.
        mirrors: bool,
    },
    /// Validate the selected runtime before showing folder choices.
    Runtime(Choices),
    /// Validate complete fresh-install input before admitting installation.
    Validate(Choices),
    /// Install the validated proposal; the adapter revalidates authority itself.
    Install(Choices),
    /// Update/repair an inspected native installation.
    Maintain(String),
    /// Reacquire launch authority and open the completed installation.
    Open,
}

/// One accepted operation; the host retains its identity through completion or cancellation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Surface-local correlation identity.
    pub id: RequestId,
    /// Typed intent, never execution authority.
    pub operation: Operation,
}

/// Successful application result, matched against the outstanding operation before navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completed {
    /// Locale accepted by the application owner.
    Locale,
    /// Installation directory inspection completed.
    Location(Location),
    /// Upstream option discovery completed.
    Options(Vec<Choice>),
    /// Runtime or complete selection validation succeeded.
    Validated,
    /// Native installation publication completed.
    Installed,
    /// Launch succeeded and its ownership transition settled.
    Opened,
}

/// Observed outcome of the original request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Successful typed result.
    Completed(Completed),
    /// Original application failure text, also retained by backend diagnostics/output.
    Failed(String),
    /// The admitted operation was cancelled.
    Cancelled,
}

/// Visible failure presentation, retaining original backend text or a localization key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Original application failure.
    Message(String),
    /// Existing installer localization key.
    Localized(sugarsubstitute_contracts::localization::installer::InstallerText),
}

/// Visible installation status independent of any timer or rendering technology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Use the localized preparingInstallation message.
    Preparing,
    /// Original localized/backend operation status.
    Message(String),
    /// Use the localized cancelling message.
    Cancelling,
}

/// Rejected native UI intent or inconsistent application response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowError {
    /// The operation is unavailable on this page or during outstanding work.
    ActionUnavailable,
    /// Unknown locale identifier.
    UnsupportedLocale,
    /// NUL or excessive field/label text.
    InvalidText,
    /// Invalid option index or excessive option inventory.
    InvalidOptions,
    /// A reply does not match the pending request identity or expected result type.
    UnexpectedReply,
    /// Surface-local correlation numbers cannot advance.
    SequenceExhausted,
}
impl std::fmt::Display for FlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Native setup flow rejected: {self:?}")
    }
}
impl std::error::Error for FlowError {}
