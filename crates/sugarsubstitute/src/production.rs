// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own explicit startup intake, admitted configuration and retained real entry sessions.
#[path = "production/backend_custody.rs"]
mod backend_custody;
#[path = "production/child.rs"]
pub(crate) mod child;
mod child_transport_diagnostic;
#[path = "production/configuration.rs"]
mod configuration;
#[path = "production/coordinator_authority.rs"]
mod coordinator_authority;
#[path = "production/coordinator_backend.rs"]
mod coordinator_backend;
mod coordinator_custody;
#[path = "production/dispatch.rs"]
mod dispatch;
#[path = "production/entry.rs"]
mod entry;
#[path = "production/entry_activation.rs"]
mod entry_activation;
#[path = "production/entry_bootstrap.rs"]
mod entry_bootstrap;
#[path = "production/entry_failure.rs"]
mod entry_failure;
#[path = "production/entry_runner.rs"]
mod entry_runner;
#[path = "production/options.rs"]
mod options;
#[path = "production/parent_lifecycle.rs"]
mod parent_lifecycle;
#[path = "production/qualification.rs"]
mod qualification;
#[path = "production/runtime.rs"]
mod runtime;

pub(crate) use runtime::execute;

#[path = "production/diagnostic_helper.rs"]
mod diagnostic_helper;
#[path = "production/incident_host.rs"]
mod incident_host;
#[path = "production/reporter_role.rs"]
mod reporter_role;

#[path = "production/recovery_supervisor.rs"]
mod recovery_supervisor;

mod profile_conflict;
