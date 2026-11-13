// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share native surface preparation, presentation and input owners across desktop lifetimes.

#[cfg(target_os = "linux")]
use crate::linux::Presenter;
#[cfg(target_os = "macos")]
use crate::macos::Presenter;
#[cfg(target_os = "windows")]
use crate::windows::Presenter;
use crate::{DesktopError, native_action::NativeAction};
use sugarsubstitute_contracts::presentation::FrameShape;
use tao::event::Event;

#[path = "runtime_input.rs"]
pub(crate) mod input;
#[path = "runtime_present.rs"]
pub(crate) mod presentation;
#[path = "runtime_state.rs"]
pub(crate) mod state;
#[path = "runtime_visibility.rs"]
pub(crate) mod visibility;

/// Validate actual device shape without scanning or touching retained pixel storage.
pub(crate) fn surface_shape(
    window: &tao::window::Window,
) -> Result<Option<FrameShape>, DesktopError> {
    let size = window.inner_size();
    if size.width == 0 || size.height == 0 {
        return Ok(None);
    }
    FrameShape::new(size.width, size.height, window.scale_factor())
        .map(Some)
        .map_err(|cause| DesktopError::new("native_frame_shape", cause))
}

/// Query opt-in move evidence around submission; portable surfaces have no Windows evidence.
pub(crate) fn observe_move(
    window: &tao::window::Window,
    auditing: bool,
) -> Result<bool, DesktopError> {
    #[cfg(target_os = "windows")]
    if auditing {
        return crate::windows_move_state::is_moving(window);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, auditing);
    Ok(false)
}

/// Classify callbacks without borrowing content or touching frame storage.
pub(crate) fn callback_kind(event: &Event<'_, NativeAction>) -> crate::timing::NativeCallbackKind {
    use crate::timing::NativeCallbackKind;
    match event {
        Event::WindowEvent { .. } => NativeCallbackKind::WindowEvent,
        Event::UserEvent(NativeAction::Completions) => NativeCallbackKind::CompletionDrain,
        // Every other native action exists only on Windows.
        #[cfg(target_os = "windows")]
        Event::UserEvent(_) => NativeCallbackKind::NativeAction,
        Event::RedrawRequested(_) => NativeCallbackKind::Redraw,
        Event::MainEventsCleared => NativeCallbackKind::LoopProgress,
        _ => NativeCallbackKind::Other,
    }
}
