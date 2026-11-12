// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe and reconcile editable native values without replacing a newer user edit.
use super::Engine;
use crate::DesktopError;
use sugarsubstitute_ui::windows::text_style::TextStyle;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT},
};

const MAXIMUM_UNITS: usize = TextStyle::Field.maximum_length() as usize;

/// Exact text and revision observed from one original field, in its native UTF-16 document.
pub(crate) struct Value {
    /// Incomplete native UTF-16 stays in the editor until another input message completes it.
    pub(crate) text: Option<String>,
    pub(crate) revision: u64,
}

impl Engine {
    /// Admit the documented native single-line replacement through the same typed value owner.
    /// The window procedure receives the native message's borrowed, terminated UTF-16 string.
    pub(super) fn field_message(&self, value: LPARAM) -> Result<bool, DesktopError> {
        let value = if value.0 == 0 {
            String::new()
        } else {
            let Some(value) = native_text(value) else {
                return Ok(false);
            };
            value
        };
        if value.contains(['\r', '\n']) {
            return Ok(false);
        }
        self.replace_field(self.state.revision.get(), &value)
    }

    /// Copy bounded single-line text and its original revision on the owning native thread.
    /// Selection/focus/undo are never changed by observation.
    pub(crate) fn field_value(&self) -> Result<Value, DesktopError> {
        self.field_live()?;
        let revision = self.state.revision.get();
        let length = self.message(WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0))?.0;
        let length = usize::try_from(length)
            .map_err(|cause| DesktopError::new("field_text_length", cause))?;
        if length > TextStyle::Field.maximum_length() as usize {
            return Err(rejected("field_text_length"));
        }
        let mut text = [0u16; MAXIMUM_UNITS + 1];
        let copied =
            self.message(WM_GETTEXT, WPARAM(length + 1), LPARAM(text.as_mut_ptr() as isize))?.0;
        if copied != length as isize || self.state.revision.get() != revision || text[length] != 0 {
            return Err(rejected("field_text_observation"));
        }
        Ok(Value { text: String::from_utf16(&text[..length]).ok(), revision })
    }

    /// Apply a model value only against its observed native revision.
    /// Returns false when a later edit makes the model stale. An unchanged value preserves
    /// native selection, scroll and undo instead of sending a document-replacement message.
    pub(crate) fn replace_field(
        &self,
        expected_revision: u64,
        value: &str,
    ) -> Result<bool, DesktopError> {
        self.field_live()?;
        let mut text = [0u16; MAXIMUM_UNITS + 1];
        for (length, unit) in value.encode_utf16().enumerate() {
            if length == TextStyle::Field.maximum_length() as usize || matches!(unit, 0 | 10 | 13) {
                return Err(rejected("field_text_input"));
            }
            text[length] = unit;
        }
        let current = self.field_value()?;
        if current.revision != expected_revision {
            return Ok(false);
        }
        if current.text.as_deref() == Some(value) {
            return Ok(true);
        }
        if self.message(WM_SETTEXT, WPARAM(0), LPARAM(text.as_ptr() as isize))?.0 == 0 {
            return Err(rejected("field_text_replace"));
        }
        if self.field_value()?.text.as_deref() != Some(value) {
            return Err(rejected("field_text_replace"));
        }
        Ok(true)
    }

    /// A field operation cannot mutate the shared read-only transcript or a retired document.
    fn field_live(&self) -> Result<(), DesktopError> {
        if self.state.style != TextStyle::Field || self.state.closing.get() {
            return Err(rejected("field_text_identity"));
        }
        self.check()
    }
}

fn rejected(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native field value or identity is invalid")
}

/// Copy only a bounded native string; malformed user input leaves the original document usable.
fn native_text(value: LPARAM) -> Option<String> {
    let pointer = value.0 as *const u16;
    let mut text = [0u16; MAXIMUM_UNITS + 1];
    for index in 0..text.len() {
        // SAFETY: WM_SETTEXT lends a terminated UTF-16 input for this synchronous call.
        // Read at most the admitted field limit plus its required terminator.
        text[index] = unsafe { pointer.add(index).read() };
        if text[index] == 0 {
            return String::from_utf16(&text[..index]).ok();
        }
    }
    None
}
