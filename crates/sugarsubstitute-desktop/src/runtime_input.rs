// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route native input while preserving caption drag and suppressing closing mutations.

#[cfg(not(windows))]
use crate::KeyAction;
use crate::{DesktopError, HitTarget, Pointer, PointerAction, WindowContent};
use tao::{
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::Key,
    window::Window,
};

/// Separate preparation dirtiness from native exposure and terminal intent.
#[derive(Default)]
pub(crate) struct InputEffect {
    pub(crate) dirty: bool,
    pub(crate) expose: bool,
    pub(crate) close: bool,
    #[cfg(windows)]
    pub(crate) focus: Option<bool>,
}

/// Forward UI-affine input; caller never invokes this once the close phase begins.
pub(crate) fn dispatch(
    content: &mut impl WindowContent,
    window: &Window,
    event: WindowEvent<'_>,
    pointer: &mut Pointer,
    pressed: &mut Option<HitTarget>,
) -> Result<InputEffect, DesktopError> {
    let mut effect = InputEffect::default();
    match event {
        WindowEvent::CloseRequested | WindowEvent::Destroyed => effect.close = true,
        WindowEvent::CursorMoved { position, .. } => {
            let scale = window.scale_factor();
            *pointer = Pointer { x: position.x / scale, y: position.y / scale };
            effect.dirty = content.pointer(PointerAction::Move, *pointer);
        }
        WindowEvent::CursorLeft { .. } | WindowEvent::Focused(false) => {
            *pressed = None;
            effect.dirty = content.pointer(PointerAction::Leave, *pointer);
        }
        WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
            let target = content.hit_test(*pointer);
            if state == ElementState::Pressed {
                *pressed = Some(target);
                effect.dirty = content.pointer(PointerAction::Press, *pointer);
                if target == HitTarget::Caption {
                    tracing::info!(
                        operation = "native_caption_press",
                        ?target,
                        "Classified caption pointer press"
                    );
                    #[cfg(target_os = "windows")]
                    crate::windows_drag::begin(window)?;
                    #[cfg(not(target_os = "windows"))]
                    window
                        .drag_window()
                        .map_err(|error| DesktopError::new("drag_window", error))?;
                    *pressed = None;
                    effect.dirty |= content.pointer(PointerAction::Leave, *pointer);
                }
            } else {
                effect.dirty = content.pointer(PointerAction::Release, *pointer);
                effect.close = *pressed == Some(HitTarget::Close) && target == HitTarget::Close;
                *pressed = None;
            }
        }
        WindowEvent::KeyboardInput { event, .. }
            if event.state == ElementState::Pressed && !event.repeat =>
        {
            match event.logical_key {
                Key::Escape => effect.close = true,
                Key::Tab => {
                    #[cfg(windows)]
                    {
                        effect.focus = Some(crate::windows_controls::shift_pressed());
                    }
                    #[cfg(not(windows))]
                    {
                        effect.dirty = content.key(KeyAction::Tab);
                    }
                }
                _ => {}
            }
        }
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => effect.dirty = true,
        WindowEvent::Focused(true) => effect.expose = true,
        _ => {}
    }
    Ok(effect)
}
