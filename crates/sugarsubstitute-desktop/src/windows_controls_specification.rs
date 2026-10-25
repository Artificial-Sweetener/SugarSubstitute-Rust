// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt public caption metadata and shared form values to one native peer inventory.
use super::*;
use sugarsubstitute_ui::form::Role;

/// The existing public button geometry retains its original f64 coordinates. Form behavior and
/// values come from the shared UI owner; native text revisions prevent stale value replacement.
#[derive(Clone)]
pub(crate) struct Specification {
    pub(crate) control: AccessibleControl,
    pub(crate) role: Role,
    pub(crate) text_revision: u64,
}
impl From<AccessibleControl> for Specification {
    fn from(control: AccessibleControl) -> Self {
        Self { control, role: Role::Button, text_revision: 0 }
    }
}
impl Specification {
    /// Borrow shared form semantics while preserving original native edit revisions.
    pub(crate) fn form(control: &sugarsubstitute_ui::form::Control, close: bool) -> Self {
        Self {
            control: AccessibleControl {
                id: control.id.get(),
                label: control.label,
                bounds: crate::ControlBounds {
                    x: control.bounds.x.into(),
                    y: control.bounds.y.into(),
                    width: control.bounds.width.into(),
                    height: control.bounds.height.into(),
                },
                close,
                visible: control.availability.visible,
                enabled: control.availability.enabled,
            },
            role: control.role.clone(),
            text_revision: control.text_revision,
        }
    }
    /// Preserve an existing public button inventory at the native adapter boundary.
    pub(crate) fn buttons(controls: &[AccessibleControl]) -> Vec<Self> {
        controls.iter().copied().map(Self::from).collect()
    }
}

/// Validate all metadata before native effects; a peer keeps its original role throughout its life.
pub(super) fn validate(
    planned: &[Specification],
    proposed: &[Specification],
) -> Result<(), DesktopError> {
    if proposed.len() > 64 || (!planned.is_empty() && planned.len() != proposed.len()) {
        return Err(DesktopError::message(
            "native_form_inventory",
            "peer inventory changed or exceeded its bound",
        ));
    }
    let mut bytes = 0;
    for (index, spec) in proposed.iter().enumerate() {
        validate_controls(std::slice::from_ref(&spec.control))?;
        values::peer_rect(spec.control, &spec.role, 1.0)?;
        if proposed[..index].iter().any(|old| old.control.id == spec.control.id)
            || spec.control.label.encode_utf16().count() > 128
            || (spec.control.close && !matches!(spec.role, Role::Button))
        {
            return Err(DesktopError::message(
                "native_form_inventory",
                "invalid peer identity or label",
            ));
        }
        bytes +=
            spec.role.validate().map_err(|cause| DesktopError::new("native_form_value", cause))?;
        if bytes > 256 * 1024 {
            return Err(DesktopError::message(
                "native_form_inventory",
                "peer values exceed admission bound",
            ));
        }
        if let Some(original) = planned.get(index) {
            state::validate_inventory(
                std::slice::from_ref(&original.control),
                std::slice::from_ref(&spec.control),
            )?;
            if std::mem::discriminant(&original.role) != std::mem::discriminant(&spec.role) {
                return Err(DesktopError::message("native_form_inventory", "peer role changed"));
            }
        }
    }
    Ok(())
}
