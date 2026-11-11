// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the Windows conflict surface and native backend with the shared controller.
use std::error::Error;
use sugarsubstitute_contracts::{
    instance_conflict::{ConflictAction, ConflictController},
    localization::Messages,
};
use sugarsubstitute_supervision::profile_conflict::{ConflictRequest, ConflictSession};

/// Preserve custody through dialog failure and return only after execution has retired.
pub(super) fn resolve(request: ConflictRequest, copy: &Messages) -> Result<bool, Box<dyn Error>> {
    let mut service =
        sugarsubstitute::instance_conflict::Service::new(ConflictSession::new(request));
    let shown = sugarsubstitute_desktop::show_startup_conflict(
        sugarsubstitute::APPLICATION_NAME,
        copy.conflict,
        &mut service,
    );
    if !service.finished() {
        service.choose(ConflictAction::Cancel);
        sugarsubstitute_desktop::run_headless(&mut service)?;
    }
    shown?;
    Ok(service.continued())
}
