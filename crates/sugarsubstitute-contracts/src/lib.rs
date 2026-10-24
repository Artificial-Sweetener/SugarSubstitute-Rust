// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define owned application contracts independently of desktop and execution adapters.

pub mod application_launch;
pub mod backend_custody;
pub mod configuration;
/// Bound independent invocation facts without native coordinator authority.
pub mod coordinator_entry;
pub mod crash;
pub mod execution;
pub mod incident;
pub mod installation;
pub mod instance_conflict;
pub mod lifecycle;
pub mod localization;
pub mod presentation;
pub mod recovery_supervisor;
pub mod startup;
pub mod transcript;
pub mod update;
