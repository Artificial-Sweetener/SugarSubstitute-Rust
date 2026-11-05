// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared native widget rendering. Callers own layout and submit bounded preparation
//! through the execution adapter; this crate does not create windows or schedule work.

pub mod accent;
pub mod availability;
/// Data-only pixel storage for shared painting and retained frame publication.
pub mod bitmap;
pub mod caption;
pub mod cursors;
pub mod form;
pub mod icons;
pub mod panel;
pub mod scene;
pub mod shapes;
pub mod text;
pub mod vector;
pub mod widgets;

pub mod progress;

#[cfg(target_os = "windows")]
pub mod windows;
