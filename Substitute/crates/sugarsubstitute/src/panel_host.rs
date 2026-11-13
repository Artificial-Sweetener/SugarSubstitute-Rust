// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what SugarSubstitute serves and declares to the embedded SugarCubes page, and the
//! presentation it configures.
//!
//! The production panel owner answers every page `ready` from these decisions, and the host
//! conformance proof composes its answers from the same functions, so the recorded host profile
//! cannot drift from what ships. Nothing here reads native state: the browser's hosting and the
//! operating-system accent are facts the caller already observed.

use sugarsubstitute_contracts::{
    appearance::AccentColor,
    host::{HostCapability, HostDescription, HostProtocol},
    panel::{PanelError, PanelPresentation, PanelStyle},
};
use sugarsubstitute_desktop::MainHosting;

/// The protocols this host answers on every main browser's panel channel.
///
/// The page's memory budget arrives as `sugarcubes.memory` allowances, so the protocol is
/// always declared; the host never reports a budget any other way. The startup host answers
/// `sugarcubes.updates` for every navigation, so it is declared too.
const PROTOCOLS: [HostProtocol; 4] =
    [HostProtocol::PANEL, HostProtocol::CANVAS, HostProtocol::MEMORY, HostProtocol::UPDATES];

/// What this host serves and declares to a page in a browser with `hosting`.
///
/// The page keeps the user's saved appearance, so `presentation.host-styles` is never declared.
/// The configured accent is the operating-system accent, offered beside a custom one. Only a
/// composition-hosted browser can show output on the native canvas; a windowed browser presents
/// output images itself, so it is not told the host presents them.
///
/// # Errors
/// Rejects a description the shared schema bounds refuse, which would be a defect here.
pub(crate) fn description(hosting: MainHosting) -> Result<HostDescription, PanelError> {
    let mut capabilities = vec![HostCapability::SYSTEM_ACCENT];
    if hosting.presents_native_canvas() {
        capabilities.push(HostCapability::HOST_OUTPUT_SURFACE);
    }
    HostDescription::new(PROTOCOLS.to_vec(), capabilities)
}

/// The SugarCubes presentation this host configures; the accent is the operating-system accent,
/// or `null` while the system reports none.
///
/// The host does not offer the PhotoPro style, so it sends no PhotoPro palette.
///
/// # Errors
/// Rejects a presentation the shared schema bounds refuse, which would be a defect here.
pub(crate) fn presentation(accent: Option<AccentColor>) -> Result<PanelPresentation, PanelError> {
    PanelPresentation::new(false, PanelStyle::Comfy, None, accent.map(AccentColor::css_hex), 0)
}

#[cfg(test)]
mod tests;
