// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt validated ComfyUI startup inputs without acquiring native process authority.

pub mod configuration;
pub mod launch;
pub mod readiness;
#[cfg(windows)]
pub mod readiness_transport;
