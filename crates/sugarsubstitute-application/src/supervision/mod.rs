// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own lease/resource and desktop-exit policy; startup presentation remains with StartupSession.
//! Native containment, authenticated IPC and OS process registries remain adapter work.
mod desktop_exit;
mod resource_observations;
mod resource_requests;
mod resource_state;
mod resources;
pub use desktop_exit::{DesktopExitSnapshot, DesktopExitTransaction};
pub use resource_state::{
    MAX_LEASES, MAX_REQUESTS, MAX_RESOURCES, MAX_RUN_REQUESTS, MAX_RUNS, REQUEST_HISTORY,
    ResourcePhase,
};
pub use resources::{CoordinatorSnapshot, ResourceCoordinator};
