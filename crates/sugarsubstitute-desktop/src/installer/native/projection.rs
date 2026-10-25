// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match prepared page geometry to one stable, clipped native peer inventory.
use super::*;
use crate::{control_geometry::physical_bounds, windows_controls::Specification};
use ::windows::Win32::Foundation::RECT;

pub(super) struct Projection {
    pub(super) specifications: Vec<Specification>,
    pub(super) clips: Vec<Option<RECT>>,
}
impl Projection {
    /// Validate every page identity before applying values or changing any original native peer.
    pub(super) fn new(
        inventory: &[Control],
        page: &Page<'_>,
        close: ControlId,
    ) -> Result<Self, Box<dyn Error>> {
        sugarsubstitute_ui::form::validate(inventory, page.frame.scale() as f32)?;
        let shape = page.frame.shape();
        let extent = Bounds {
            x: 0.0,
            y: 0.0,
            width: shape.width() as f32 / shape.scale() as f32,
            height: shape.height() as f32 / shape.scale() as f32,
        };
        page.viewport.validate(shape.scale() as f32)?;
        page.drag_bounds.validate(shape.scale() as f32)?;
        if !contains(extent, page.drag_bounds) {
            return Err("Native caption escapes its frame".into());
        }
        if !contains(extent, page.viewport) {
            return Err("Native page viewport escapes its frame".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for control in page.shell.iter().chain(page.controls) {
            if !seen.insert(control.id)
                || !inventory.iter().any(|original| {
                    original.id == control.id
                        && std::mem::discriminant(&original.role)
                            == std::mem::discriminant(&control.role)
                })
            {
                return Err("Native page changed its original peer inventory".into());
            }
        }
        let mut specifications = Vec::with_capacity(inventory.len());
        let mut clips = Vec::with_capacity(inventory.len());
        for original in inventory {
            let shell = page.shell.iter().find(|item| item.id == original.id);
            let content = page.controls.iter().find(|item| item.id == original.id);
            let mut value = shell.or(content).unwrap_or(original).clone();
            value.availability.visible &= shell.is_some() || content.is_some();
            let clip = if content.is_some() {
                value.bounds.x += page.viewport.x;
                value.bounds.y += page.viewport.y;
                value.availability.visible &= intersects(value.bounds, page.viewport);
                let mut viewport = Specification::form(&value, false).control;
                viewport.bounds = crate::ControlBounds {
                    x: page.viewport.x.into(),
                    y: page.viewport.y.into(),
                    width: page.viewport.width.into(),
                    height: page.viewport.height.into(),
                };
                Some(physical_bounds(&viewport, shape.scale())?)
            } else {
                None
            };
            if value.availability.visible && !contains(extent, value.bounds) {
                return Err("Visible native peer escapes its prepared frame".into());
            }
            if !value.availability.visible {
                value.bounds = original.bounds;
            }
            specifications.push(Specification::form(&value, value.id == close));
            clips.push(clip);
        }
        Ok(Self { specifications, clips })
    }
    /// Pointer input follows the same clipping and native role as the painted page.
    pub(super) fn hit(&self, x: f64, y: f64, scale: f64) -> Option<ControlId> {
        self.specifications.iter().zip(&self.clips).rev().find_map(|(spec, clip)| {
            let b = spec.control.bounds;
            let inside = x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height;
            let clipped = clip.is_some_and(|r| {
                x * scale < r.left.into()
                    || x * scale >= r.right.into()
                    || y * scale < r.top.into()
                    || y * scale >= r.bottom.into()
            });
            (inside
                && !clipped
                && spec.control.availability().actionable()
                && !matches!(
                    spec.role,
                    sugarsubstitute_ui::form::Role::TextField(_)
                        | sugarsubstitute_ui::form::Role::Select { .. }
                ))
            .then(|| ControlId::new(spec.control.id).ok())
            .flatten()
        })
    }
}
/// Require an entire logical rectangle to stay inside its original prepared surface.
fn contains(outer: Bounds, inner: Bounds) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}
/// Preserve partial visibility while excluding controls entirely outside the page viewport.
fn intersects(a: Bounds, b: Bounds) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}
