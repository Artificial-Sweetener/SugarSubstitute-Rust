// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy only visible installation facts under admission before transferring preparation to a worker.
use super::*;
use sugarsubstitute_application::setup_flow::{ExistingInstallation, Failure, Status};

#[cfg(test)]
mod tests;

/// Upper bound for one immutable page input, including all bounded upstream display lists.
pub const SNAPSHOT_BYTES: u64 = 512 * 1024;

/// Native rendering receives display text and indices, never upstream provisioning tokens.
pub(super) struct Options {
    pub(super) labels: Vec<String>,
    pub(super) selected: Option<usize>,
}

/// User-entered paths copied verbatim for native display and editing.
pub(super) struct Paths {
    pub(super) installation: String,
    pub(super) comfy: String,
    pub(super) python: String,
    pub(super) models: String,
    pub(super) outputs: String,
}

/// Immutable preparation input. Native and application state can advance while this copy renders.
/// Its original memory admission remains owned until worker preparation releases the snapshot.
pub struct Snapshot {
    pub(super) view: View,
    pub(super) page: Page,
    pub(super) locale: &'static str,
    pub(super) selected_locale: &'static str,
    pub(super) mode: RuntimeMode,
    pub(super) paths: Paths,
    pub(super) existing: Option<ExistingInstallation>,
    options: [Options; 3],
    pub(super) variant_description: String,
    pub(super) mirrors: bool,
    pub(super) overall: f64,
    pub(super) status: Status,
    pub(super) failure: Option<Failure>,
    pub(super) details_open: bool,
    pub(super) closing: bool,
    pub(super) journey: (usize, usize),
    pub(super) primary_key: Option<Text>,
    pub(super) can_primary: bool,
    pub(super) can_back: bool,
    pub(super) can_edit: bool,
    pub(super) waiting: bool,
    _claim: MemoryClaim,
}

impl Snapshot {
    /// Claim bounded metadata before copying the application owner's current presentation facts.
    /// No font discovery, measurement, rasterization or native call occurs on this path.
    /// # Errors
    /// Preserves insufficient admission before allocating any copied text or option list.
    pub fn capture(flow: &SetupFlow, view: View, lease: &MemoryLease) -> io::Result<Self> {
        let claim = lease.claim(SNAPSHOT_BYTES).map_err(io::Error::other)?;
        let choices = flow.choices();
        let options =
            [OptionField::Release, OptionField::ComfyVersion, OptionField::Variant].map(|field| {
                Options {
                    labels: flow.options(field).iter().map(|choice| choice.label.clone()).collect(),
                    selected: flow.selected(field),
                }
            });
        let variant_description = flow
            .selected(OptionField::Variant)
            .and_then(|index| flow.options(OptionField::Variant).get(index))
            .map(|choice| choice.description.clone())
            .unwrap_or_default();
        Ok(Self {
            view,
            page: flow.page(),
            locale: flow.locale(),
            selected_locale: choices.locale,
            mode: choices.mode,
            paths: Paths {
                installation: choices.install_root.clone(),
                comfy: choices.comfy_root.clone(),
                python: choices.python.clone(),
                models: choices.models.clone(),
                outputs: choices.outputs.clone(),
            },
            existing: flow.existing().cloned(),
            options,
            variant_description,
            mirrors: flow.mirrors(),
            overall: flow.overall(),
            status: flow.status().clone(),
            failure: flow.failure().cloned(),
            details_open: flow.details_open(),
            closing: flow.is_closing(),
            journey: flow.journey(),
            primary_key: flow.primary_key(),
            can_primary: flow.can_primary(),
            can_back: flow.can_back(),
            can_edit: flow.can_edit(),
            waiting: flow.waiting(view.now),
            _claim: claim,
        })
    }

    /// Resolve a display list by its original application field, without rebuilding selection policy.
    pub(super) fn options(&self, field: OptionField) -> &Options {
        match field {
            OptionField::Release => &self.options[0],
            OptionField::ComfyVersion => &self.options[1],
            OptionField::Variant => &self.options[2],
        }
    }
}
