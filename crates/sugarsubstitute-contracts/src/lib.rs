// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define owned application contracts independently of desktop and execution adapters.

pub mod configuration;
/// Bound independent invocation facts without native coordinator authority.
pub mod coordinator_entry;
pub mod crash;
pub mod execution;
pub mod incident;
pub mod lifecycle;
pub mod localization;
pub mod presentation;
pub mod startup;
pub mod transcript;
