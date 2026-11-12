// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt validated ComfyUI startup inputs without acquiring native process authority.

pub mod launch;
/// Construct bounded Comfy output requests without accepting host-supplied destinations.
pub mod output_media;
/// Follow ComfyUI's prompt queue over its WebSocket.
pub mod queue_watch;
pub mod readiness;
#[cfg(windows)]
pub mod readiness_campaign;
#[cfg(windows)]
pub mod readiness_transport;
pub mod startup_feedback;
