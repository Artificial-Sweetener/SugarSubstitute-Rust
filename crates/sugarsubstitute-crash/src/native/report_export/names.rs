// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve selected spelling and form bounded collision-resistant sibling names.
use super::*;
/// Form fixed selected-name siblings before any native opening effect.
pub(super) fn prepare(fields: &mut Fields, intent: &ReportExportIntent) -> Result<(), E> {
    let prefix = [92, 92, 63, 92];
    fields.target_path[..4].copy_from_slice(&prefix);
    fields.target_path[4..intent.length + 4].copy_from_slice(&intent.path[..intent.length]);
    fields.target_length = intent.length + 4;
    siblings(fields, intent)
}
/// Bind descendants to the Volume-GUID root returned from the actual held root handle.
/// Reject SUBST roots and other non-volume aliases before any descendant creation.
pub(super) fn rebase(
    fields: &mut Fields,
    intent: &ReportExportIntent,
    root: &[u16],
) -> Result<(), E> {
    let prefix = [92, 92, 63, 92, 86, 111, 108, 117, 109, 101, 123];
    if root.len() != 49 || !root.starts_with(&prefix) || root[47] != 125 || root[48] != 92 {
        return Err(E::Rejected(C::Destination));
    }
    let length = root.len() + intent.length - 3;
    if length >= fields.target_path.len() {
        return Err(E::Rejected(C::Destination));
    }
    fields.target_path.fill(0);
    fields.target_path[..root.len()].copy_from_slice(root);
    fields.target_path[root.len()..length].copy_from_slice(&intent.path[3..intent.length]);
    fields.target_length = length;
    fields.root_length = root.len();
    siblings(fields, intent)
}
/// Rebuild stage and backup names from the same actual canonical selected target.
fn siblings(fields: &mut Fields, intent: &ReportExportIntent) -> Result<(), E> {
    for (buffer, suffix) in [
        (&mut fields.stage_path, b"stage".as_slice()),
        (&mut fields.backup_path, b"prior".as_slice()),
    ] {
        buffer.fill(0);
        buffer[..fields.target_length].copy_from_slice(&fields.target_path[..fields.target_length]);
        let mut position = fields.target_length;
        push(buffer, &mut position, b".sugar-")?;
        for value in [intent.binding().session().get(), intent.request().get()] {
            for shift in (0..16).rev() {
                let digit = ((value >> (shift * 4)) & 15) as usize;
                push(buffer, &mut position, &[b"0123456789abcdef"[digit]])?;
            }
            push(buffer, &mut position, b"-")?;
        }
        push(buffer, &mut position, suffix)?;
        let leaf =
            buffer[..position].iter().rposition(|u| *u == 92).ok_or(E::Rejected(C::Destination))?;
        if position - leaf - 1 > 255 {
            return Err(E::Rejected(C::Destination));
        }
    }
    Ok(())
}
/// Append fixed ASCII suffix data while reserving the terminating UTF16 NUL.
fn push(output: &mut [u16], position: &mut usize, bytes: &[u8]) -> Result<(), E> {
    let end = position
        .checked_add(bytes.len())
        .filter(|n| *n < output.len())
        .ok_or(E::Rejected(C::Destination))?;
    for (unit, byte) in output[*position..end].iter_mut().zip(bytes) {
        *unit = u16::from(*byte);
    }
    *position = end;
    Ok(())
}
