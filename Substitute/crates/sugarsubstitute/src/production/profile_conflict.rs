// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the Windows conflict surface and native backend with the shared controller.
mod instance;
use super::entry_runner::{PreviousCopyBusy, PreviousCopyState};
use std::error::Error;
use sugarsubstitute_contracts::{
    instance_conflict::{ConflictAction, ConflictController},
    localization::Messages,
};
use sugarsubstitute_supervision::profile_conflict::{ConflictRequest, ConflictSession};

/// Preserve custody through dialog failure and return only after execution has retired.
pub(super) fn resolve(
    request: ConflictRequest,
    copy: &'static Messages,
) -> Result<bool, Box<dyn Error>> {
    let mut service =
        sugarsubstitute::instance_conflict::Service::new(ConflictSession::new(request));
    let shown = sugarsubstitute_desktop::show_startup_conflict(
        copy,
        sugarsubstitute::APPLICATION_NAME,
        &mut service,
    );
    if !service.finished() {
        service.choose(ConflictAction::Cancel);
        sugarsubstitute_desktop::run_headless(&mut service)?;
    }
    shown?;
    Ok(service.continued())
}

/// Ask about an unresponsive running copy, or wait on screen for a closing one, then start a
/// fresh launch once it has gone. Returns whether that launch started. Custody is retired
/// through surface failure before returning.
pub(super) fn resolve_previous_copy(
    busy: PreviousCopyBusy,
    copy: &'static Messages,
) -> Result<bool, Box<dyn Error>> {
    let mut service = sugarsubstitute::instance_conflict::Service::new(
        instance::InstanceSession::new(busy.copy, busy.state),
    );
    let shown = match busy.state {
        PreviousCopyState::Unresponsive => sugarsubstitute_desktop::show_startup_conflict(
            copy,
            sugarsubstitute::APPLICATION_NAME,
            &mut service,
        ),
        PreviousCopyState::Closing => sugarsubstitute_desktop::show_previous_copy_closing(
            copy,
            sugarsubstitute::APPLICATION_NAME,
            &mut service,
        ),
    };
    if !service.finished() {
        service.choose(ConflictAction::Cancel);
        sugarsubstitute_desktop::run_headless(&mut service)?;
    }
    shown?;
    Ok(service.continued())
}
