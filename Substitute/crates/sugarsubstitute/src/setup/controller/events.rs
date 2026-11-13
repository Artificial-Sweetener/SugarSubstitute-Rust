// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Owned backend results shared by native navigation and provisioning qualification.
use super::*;
use sugarsubstitute_application::setup_flow::{self as flow, Choice};

mod wire;
pub(super) use wire::options;

/// Original operation identity, retained independently of whichever surface consumes the result.
pub(super) struct Event {
    pub(super) id: Option<String>,
    pub(super) kind: Kind,
}

/// Observed installation facts and diagnostics, without presentation-driven authority.
pub(super) enum Kind {
    Completed(Completed),
    Failed(String),
    Cancelled(Option<String>),
    Progress(Progress),
    Diagnostic(Diagnostic),
    Unavailable(String),
}

/// Retain the diagnostic owner's bounded redacted projection without worker-asserted trust.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Diagnostic {
    pub(super) message: String,
    pub(super) omitted: u64,
}
impl Diagnostic {
    /// Project untrusted or original developer evidence before retaining a presentation copy.
    pub(super) fn project(value: &Value) -> Result<Self> {
        Ok(serde_json::from_value(crate::diagnostics::worker::project(value))?)
    }
}

/// Worker phase details remain available for diagnostics and cancellation qualification.
pub(super) struct Progress {
    pub(super) status: String,
    pub(super) overall: f64,
    pub(super) phase: Option<String>,
    pub(super) percent: Option<f64>,
}
impl Progress {
    /// A storage-owned phase has overall progress without upstream phase measurements.
    pub(super) fn storage(status: String, overall: f64) -> Self {
        Self { status, overall, phase: None, percent: None }
    }
}

/// Storage retains full inspection facts until the presenting boundary selects its display data.
pub(super) enum Completed {
    Locale,
    Location { root: PathBuf, existing: Option<Box<setup_storage::InstallationSummary>> },
    Options(Vec<Choice>),
    Validated,
    Installed,
    Opened,
}

impl Completed {
    /// Strip storage-only facts before applying a result to native navigation.
    /// Rejects unrepresentable paths instead of changing the user's installation identity.
    pub(super) fn into_flow(self) -> Result<flow::Completed> {
        Ok(match self {
            Self::Locale => flow::Completed::Locale,
            Self::Location { root, existing } => flow::Completed::Location(flow::Location {
                comfy_root: path_text(root.join("ComfyUI"))?,
                models: path_text(root.join("user/models"))?,
                outputs: path_text(root.join("user/outputs"))?,
                existing: existing
                    .map(|existing| {
                        Ok::<_, Box<dyn Error>>(flow::ExistingInstallation {
                            install_root: path_text(existing.selection.install_root)?,
                            comfy_root: path_text(existing.selection.comfy_root)?,
                            executable_missing: existing.executable_missing,
                            entry_missing: existing.launch_entry_missing,
                        })
                    })
                    .transpose()?,
            }),
            Self::Options(options) => flow::Completed::Options(options),
            Self::Validated => flow::Completed::Validated,
            Self::Installed => flow::Completed::Installed,
            Self::Opened => flow::Completed::Opened,
        })
    }
}

/// Convert a native path only when it is exactly representable by the shared text engine.
pub(super) fn path_text(path: PathBuf) -> Result<String> {
    path.into_os_string().into_string().map_err(|_| "Installation path is not Unicode".into())
}

impl Controller {
    /// Queue bounded owned results and append diagnostic text through the existing console owner.
    pub(super) fn emit(&mut self, id: Option<String>, kind: Kind) -> Result<()> {
        if self.output.len() >= 32 {
            return Err("Installer response admission exceeded".into());
        }
        let text = match &kind {
            Kind::Progress(progress) => Some(&progress.status),
            Kind::Failed(message) | Kind::Unavailable(message) => Some(message),
            Kind::Diagnostic(diagnostic) => Some(&diagnostic.message),
            _ => None,
        };
        if let (Some(console), Some(text)) = (&mut self.console, text) {
            console.append(text)?;
        }
        self.output.push_back(Event { id, kind });
        Ok(())
    }
}

impl Controller {
    /// Retain original developer evidence independently of localized product feedback.
    pub(super) fn diagnostic(&mut self, message: String) -> Result<()> {
        self.emit(None, Kind::Diagnostic(Diagnostic::project(&json!({"message":message}))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_location_keeps_authored_data_under_user() -> Result<()> {
        let root = PathBuf::from(r"E:\SugarSubstitute");
        let flow::Completed::Location(location) =
            (Completed::Location { root: root.clone(), existing: None }).into_flow()?
        else {
            return Err("Location result changed kind".into());
        };
        assert_eq!(PathBuf::from(location.comfy_root), root.join("ComfyUI"));
        assert_eq!(PathBuf::from(location.models), root.join("user/models"));
        assert_eq!(PathBuf::from(location.outputs), root.join("user/outputs"));
        Ok(())
    }
}
