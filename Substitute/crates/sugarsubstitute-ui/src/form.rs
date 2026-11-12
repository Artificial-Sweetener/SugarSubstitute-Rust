// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Immutable native form peers. Surface owners supply values; desktop adapters own native handles.
use crate::{availability::Availability, widgets::Bounds};
use std::{io, num::NonZeroU16};

pub mod selection;

/// Logical space reserved by scrollable Fluent forms: the shared scrollbar and its text clearance.
pub const SCROLL_GUTTER: f32 = 15.0;

/// Native editable glyph viewport inside the shared field decoration, in logical coordinates.
/// # Errors
/// Rejects fields too small to contain the original 19-pixel text line and horizontal padding.
pub fn field_viewport(bounds: Bounds) -> io::Result<Bounds> {
    let inner = Bounds {
        x: bounds.x + 12.0,
        y: bounds.y + (bounds.height - 19.0) * 0.5,
        width: bounds.width - 24.0,
        height: 19.0,
    };
    if bounds.height < 19.0 {
        return Err(io::Error::other("Native field is shorter than its text line"));
    }
    inner.validate(1.0).map_err(io::Error::other)?;
    Ok(inner)
}

/// Stable identity within one native form, independent of its current page or placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ControlId(NonZeroU16);
impl ControlId {
    /// Preserve an already validated nonzero identity supplied by a typed surface action.
    pub const fn from_nonzero(value: NonZeroU16) -> Self {
        Self(value)
    }
    /// Reject the reserved native control identity before creating any peer.
    /// # Errors
    /// Returns invalid input for zero.
    pub fn new(value: u16) -> io::Result<Self> {
        NonZeroU16::new(value).map(Self).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Zero form control identity")
        })
    }
    /// Native control identifier; application intent is resolved by the surface owner.
    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// Accessible native role and its current value. These values never authorize application work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// Invoke one application action.
    Button,
    /// Edit one plain Unicode line with the shared field's 4096 UTF-16-unit limit.
    TextField(String),
    /// Select an index from the current bounded display list.
    Select {
        /// Original display labels, without protocol identifiers or authority tokens.
        labels: Vec<String>,
        /// Current selection; absence represents an empty/unselected list.
        selected: Option<usize>,
    },
    /// Mutually exclusive choice; the surface owner owns the group relationship.
    Radio(bool),
    /// Independent on/off choice.
    Check(bool),
}

impl Role {
    /// Validate the native value and return its retained UTF-8 content size. Native adapters use
    /// the same rules when they combine a form role with an existing public button contract.
    /// # Errors
    /// Rejects malformed text, oversized lists and selections outside their actual display list.
    pub fn validate(&self) -> io::Result<usize> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "Invalid native form value");
        let bytes = match self {
            Self::TextField(value) => {
                if !valid_text(value) || value.contains(['\r', '\n']) {
                    return Err(invalid());
                }
                value.len()
            }
            Self::Select { labels, selected } => {
                if labels.len() > 256 || selected.is_some_and(|index| index >= labels.len()) {
                    return Err(invalid());
                }
                let mut bytes = 0;
                for label in labels {
                    if !valid_text(label) {
                        return Err(invalid());
                    }
                    bytes += label.len();
                    if bytes > 256 * 1024 {
                        return Err(invalid());
                    }
                }
                bytes
            }
            Self::Button | Self::Radio(_) | Self::Check(_) => 0,
        };
        Ok(bytes)
    }
}

/// An observation from the original native peer. Presentation values grant no application authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// Invoke an application-owned button, checkbox or radio action.
    Invoke,
    /// Observe complete Unicode text, or an unfinished UTF-16 edit, at its original document revision.
    Text {
        /// Complete Unicode value; absence retains an unfinished native UTF-16 edit.
        value: Option<String>,
        /// Monotonically increasing revision of the original native document.
        revision: u64,
    },
    /// Observe the selected index within the original native selector's current list.
    Select(Option<usize>),
}

/// One original native peer's immutable presentation snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    /// Stable identity across page changes and asynchronous repaint preparation.
    pub id: ControlId,
    /// Localized accessible name, separate from a text field's user-authored value.
    pub label: &'static str,
    /// Logical bounds relative to the owning surface's client origin.
    pub bounds: Bounds,
    /// Shared visibility, input, focus and accessibility admission.
    pub availability: Availability,
    /// Native behavior and value.
    pub role: Role,
    /// Revision of the original editable document observed before this snapshot was prepared.
    /// A native adapter must not replace a newer edit with this snapshot's text.
    pub text_revision: u64,
}

/// Validate the whole peer inventory before any native identity or value is changed.
/// # Errors
/// Rejects duplicate identities, excessive values, malformed selections and invalid geometry.
pub fn validate(controls: &[Control], scale: f32) -> io::Result<()> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "Invalid native form inventory");
    if controls.len() > 64 {
        return Err(invalid());
    }
    let mut retained = 0usize;
    for (index, control) in controls.iter().enumerate() {
        control.bounds.validate(scale).map_err(io::Error::other)?;
        if controls[..index].iter().any(|prior| prior.id == control.id)
            || control.label.is_empty()
            || !valid_text(control.label)
        {
            return Err(invalid());
        }
        retained += control.label.len();
        retained += control.role.validate()?;
        if retained > 256 * 1024 {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Same bound as the actual native plain-text field; labels cannot contain a native terminator.
fn valid_text(value: &str) -> bool {
    !value.contains('\0') && value.encode_utf16().take(4097).count() <= 4096
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field() -> io::Result<Control> {
        Ok(Control {
            id: ControlId::new(1)?,
            label: "Path",
            bounds: Bounds { x: 12.0, y: 12.0, width: 300.0, height: 33.0 },
            availability: Availability { visible: true, enabled: true },
            role: Role::TextField(String::new()),
            text_revision: 0,
        })
    }
    #[test]
    fn native_form_rejects_identity_and_selection_ambiguity() -> io::Result<()> {
        assert!(ControlId::new(0).is_err());
        let control = field()?;
        validate(std::slice::from_ref(&control), 1.0)?;
        assert!(validate(&[control.clone(), control.clone()], 1.0).is_err());
        let mut select = control;
        select.role = Role::Select { labels: vec!["One".into()], selected: Some(1) };
        assert!(validate(&[select], 1.0).is_err());
        Ok(())
    }
    #[test]
    fn native_form_preserves_unicode_and_checks_real_field_limit() -> io::Result<()> {
        let mut control = field()?;
        control.role = Role::TextField("🙂".repeat(2048));
        validate(std::slice::from_ref(&control), 1.0)?;
        control.role = Role::TextField("🙂".repeat(2049));
        assert!(validate(std::slice::from_ref(&control), 1.0).is_err());
        control.role = Role::TextField("a\0b".into());
        assert!(validate(&[control], 1.0).is_err());
        Ok(())
    }

    #[test]
    fn native_field_viewport_preserves_authored_padding_and_single_line_bounds() -> io::Result<()> {
        let outer = field()?.bounds;
        let inner = field_viewport(outer)?;
        assert_eq!(inner, Bounds { x: 24.0, y: 19.0, width: 276.0, height: 19.0 });
        assert!(field_viewport(Bounds { height: 18.0, ..outer }).is_err());
        assert!(field_viewport(Bounds { width: 24.0, ..outer }).is_err());
        assert!(Role::TextField("line\nbreak".into()).validate().is_err());
        Ok(())
    }
}
