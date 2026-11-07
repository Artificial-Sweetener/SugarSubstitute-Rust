// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply shared form values to their original native control, retaining document edit custody.
use super::*;
use crate::windows_callbacks::InputKind;
use sugarsubstitute_ui::form::Role;
use windows::Win32::{
    Foundation::{HMODULE, LPARAM, WPARAM},
    UI::WindowsAndMessaging::*,
};

/// Native behavior selects its actual OS role; the shared prepared scene supplies decoration.
pub(super) fn native_style(mode: PaintMode, spec: &Specification) -> (PCWSTR, WINDOW_STYLE) {
    let base = retry::initial_style(mode, spec.control.id, spec.control.availability());
    let cleared = WINDOW_STYLE(base.0 & !(BS_TYPEMASK as u32));
    match spec.role {
        Role::Button => (w!("BUTTON"), base),
        Role::Radio(_) => (w!("BUTTON"), cleared | WINDOW_STYLE(BS_RADIOBUTTON as u32)),
        Role::Check(_) => (w!("BUTTON"), cleared | WINDOW_STYLE(BS_CHECKBOX as u32)),
        Role::TextField(_) => (w!("STATIC"), cleared),
        Role::Select { .. } => {
            (w!("COMBOBOX"), cleared | WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_VSCROLL)
        }
    }
}
/// Keyboard, paint and pointer routing follows the actual native input implementation.
pub(super) fn input(role: &Role) -> InputKind {
    match role {
        Role::TextField(_) => InputKind::Text,
        Role::Select { .. } => InputKind::Select,
        _ => InputKind::Prepared,
    }
}

/// Initialize values only after the transaction adopted both the HWND and any text engine.
pub(super) fn initialize(peer: &Peer, callback: &Callback) -> Result<(), DesktopError> {
    match &peer.role {
        Role::TextField(value) => {
            let engine = peer.engine.as_ref().ok_or_else(|| missing("initialize_field"))?;
            let revision = engine.field_value()?.revision;
            if !engine.replace_field(revision, value)? {
                return Err(missing("initialize_field"));
            }
        }
        Role::Select { labels, selected } => {
            accessibility::name(peer, peer.snapshot.label)?;
            callback.replace_selection(peer.hwnd)?;
            select(peer.hwnd, Some(labels), *selected)?;
        }
        Role::Radio(checked) | Role::Check(checked) => checked_value(peer.hwnd, *checked)?,
        Role::Button => {}
    }
    Ok(())
}

/// Preserve native selection/undo when text is unchanged and refuse a stale render's replacement.
pub(super) fn apply(
    peer: &mut Peer,
    spec: &Specification,
    module: Option<HMODULE>,
    callback: &Callback,
) -> Result<(), DesktopError> {
    match &spec.role {
        Role::TextField(value) => {
            let engine = peer.engine.as_ref().ok_or_else(|| missing("update_field"))?;
            if spec.control.label != peer.snapshot.label {
                engine.rename(
                    module.ok_or_else(|| missing("update_field_label"))?,
                    spec.control.label,
                )?;
            }
            if !engine.replace_field(spec.text_revision, value)? {
                return Ok(());
            }
        }
        Role::Select { labels, selected } => {
            if spec.control.label != peer.snapshot.label {
                accessibility::name(peer, spec.control.label)?;
            }
            let changed =
                !matches!(&peer.role, Role::Select { labels: previous, .. } if previous == labels);
            let selected_changed =
                !matches!(&peer.role,Role::Select {selected: previous,..} if previous==selected);
            if changed || (selected_changed && needs_selection(callback, peer.hwnd, *selected)?) {
                callback.replace_selection(peer.hwnd)?;
                select(peer.hwnd, changed.then_some(labels.as_slice()), *selected)?;
            }
        }
        Role::Radio(checked) | Role::Check(checked) => checked_value(peer.hwnd, *checked)?,
        Role::Button => {}
    }
    peer.role = spec.role.clone();
    Ok(())
}

/// Retain application selection policy: native radio/check peers report clicks without auto-toggle.
fn checked_value(hwnd: HWND, checked: bool) -> Result<(), DesktopError> {
    // SAFETY: Caller retains the original button on its creating thread; messages carry scalars.
    let observed = unsafe {
        SendMessageW(hwnd, BM_SETCHECK, Some(WPARAM(usize::from(checked))), Some(LPARAM(0)));
        SendMessageW(hwnd, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0)))
    };
    if observed.0 != isize::from(checked) {
        return Err(missing("native_check_value"));
    }
    Ok(())
}

/// Replace bounded display labels only when their list changes; opening a list performs no backend work.
fn select(
    hwnd: HWND,
    labels: Option<&[String]>,
    selected: Option<usize>,
) -> Result<(), DesktopError> {
    // SAFETY: Close only this original popup before replacing its authoritative list or value.
    unsafe { SendMessageW(hwnd, CB_SHOWDROPDOWN, Some(WPARAM(0)), None) };
    if let Some(labels) = labels {
        // SAFETY: Reset only this retained native selector; no pointer escapes the call.
        unsafe { SendMessageW(hwnd, CB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0))) };
        for (index, label) in labels.iter().enumerate() {
            let label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            // SAFETY: The selector synchronously copies this bounded UTF-16 display label.
            let result = unsafe {
                SendMessageW(
                    hwnd,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(label.as_ptr() as isize)),
                )
            };
            if result.0 != index as isize {
                return Err(missing("native_select_labels"));
            }
        }
    }
    let expected = selected.map_or(-1, |index| index as isize);
    // SAFETY: Scalar selection affects only the original retained native selector.
    let actual = unsafe {
        SendMessageW(hwnd, CB_SETCURSEL, Some(WPARAM(expected as usize)), Some(LPARAM(0)));
        SendMessageW(hwnd, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)))
    };
    if actual.0 != expected {
        return Err(missing("native_select_value"));
    }
    Ok(())
}
fn missing(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native peer value could not be reconciled")
}

/// Keep the native editable viewport inside the authoritative shared decoration.
pub(super) fn peer_rect(
    control: AccessibleControl,
    role: &Role,
    scale: f64,
) -> Result<RECT, DesktopError> {
    let mut control = control;
    if matches!(role, Role::TextField(_)) {
        let b = control.bounds;
        let inner = sugarsubstitute_ui::form::field_viewport(sugarsubstitute_ui::widgets::Bounds {
            x: b.x as f32,
            y: b.y as f32,
            width: b.width as f32,
            height: b.height as f32,
        })
        .map_err(|cause| DesktopError::new("native_field_viewport", cause))?;
        control.bounds = crate::ControlBounds {
            x: inner.x.into(),
            y: inner.y.into(),
            width: inner.width.into(),
            height: inner.height.into(),
        };
    }
    physical_bounds(&control, scale)
}

impl Controls {
    /// Retain exact prepared backgrounds for native text painting; fields keep their own document,
    /// selection and caret. This runs on the window's creating thread after frame acceptance.
    pub(crate) fn present_fields(
        &self,
        frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
        dark: bool,
    ) -> Result<(), DesktopError> {
        self.callback.theme.set(if dark {
            sugarsubstitute_contracts::configuration::Theme::Dark
        } else {
            sugarsubstitute_contracts::configuration::Theme::Light
        });
        for slot in &self.creation.peers {
            let peer = &slot.value;
            if let Some(engine) = &peer.engine {
                if peer.snapshot.visible {
                    engine.prepare(frame, None, peer.rect, dark, self.window.scale_factor())?;
                } else {
                    engine.hide_presentation(frame)?;
                }
            }
        }
        Ok(())
    }
}

impl Controls {
    /// Read only an original actionable peer. Native text values carry their exact revision;
    /// an incomplete UTF-16 edit remains with the engine until it becomes valid Unicode.
    pub(crate) fn observe(
        &self,
        id: u16,
    ) -> Result<Option<sugarsubstitute_ui::form::Input>, DesktopError> {
        use sugarsubstitute_ui::form::Input;
        if !self.creation.writable() {
            return Ok(None);
        }
        let Some(peer) = self
            .creation
            .peers
            .iter()
            .map(|slot| &slot.value)
            .find(|peer| peer.snapshot.id == id && peer.snapshot.availability().actionable())
        else {
            return Ok(None);
        };
        self.original_peer(peer)?;
        let value = match &peer.role {
            Role::TextField(_) => {
                let value =
                    peer.engine.as_ref().ok_or_else(|| missing("observe_field"))?.field_value()?;
                Input::Text { value: value.text, revision: value.revision }
            }
            Role::Select { labels, .. } => {
                // SAFETY: Query the original retained native selector on its creating thread.
                let index = unsafe { SendMessageW(peer.hwnd, CB_GETCURSEL, None, None) }.0;
                if index < -1 || index >= labels.len() as isize {
                    return Err(missing("observe_selection"));
                }
                Input::Select((index >= 0).then_some(index as usize))
            }
            _ => Input::Invoke,
        };
        Ok(Some(value))
    }
    /// Observe only an original visible and enabled control on this UI thread.
    pub(crate) fn focused(&self) -> Option<sugarsubstitute_ui::form::ControlId> {
        // SAFETY: Passive query does not move focus or touch another thread's window.
        let focused = unsafe { GetFocus() };
        self.creation
            .peers
            .iter()
            .map(|slot| &slot.value)
            .find(|peer| peer.hwnd == focused && peer.snapshot.availability().actionable())
            .and_then(|peer| std::num::NonZeroU16::new(peer.snapshot.id))
            .map(sugarsubstitute_ui::form::ControlId::from_nonzero)
    }
}

/// Acknowledging the committed value must preserve a newer local popup preview.
fn needs_selection(
    callback: &Callback,
    hwnd: HWND,
    selected: Option<usize>,
) -> Result<bool, DesktopError> {
    if let Some(original) = callback.selection_origin(hwnd)? {
        return Ok(original != selected);
    }
    // SAFETY: Observe only the original retained selector after releasing its callback-state borrow.
    let current = unsafe { SendMessageW(hwnd, CB_GETCURSEL, None, None) }.0;
    if current < -1 {
        return Err(missing("native_select_observation"));
    }
    Ok((current >= 0).then_some(current as usize) != selected)
}
