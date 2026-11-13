// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed observational canvas preparation facts; no pixels, paths or execution authority.
use crate::canvas_surface_diagnostic::tokens;

tokens! {
    /// First application guard or actual owned work preventing a drawable selection.
    Phase { Closing, Failed, Initializing, Retiring, Routing, Suspended, Hidden, Graphics,
        Rendering, Displaying, Pacer, Source, StaleEvidence, RenderAdmission, DisplayAdmission, DisplayBuffers }
}
tokens! {
    /// Actual execution state, independent of a cancellation request.
    Work { None, Queued, Running, Terminal }
}
tokens! {
    /// Exact pacer custody observed without submitting or accepting a frame.
    Pacer { Ready, Waiting, Preparing, AwaitingNative, Fault }
}

/// Session facts in bit order: drawable, resources, current source, acquiring, suspended,
/// route pending, document transaction, retiring, acquisition retiring, GPU initialized,
/// graphics pending, CPU pending, GPU pending, current backing, projection, current evidence,
/// matching material, render cancellation, display cancellation, borrowed render backing,
/// failed, picker, preview pending, input shown, material active, superseded rendering,
/// paired display extent present, paired display extent equals the current physical viewport.
pub(crate) const FLAGS: u32 = (1 << 28) - 1;

/// Inert snapshot copied from native owners without consuming a job, slot or completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativePreparationSnapshot {
    pub(crate) phase: Phase,
    pub(crate) flags: u32,
    /// Current scene source revision, or zero without a loaded scene.
    pub(crate) scene_source: u64,
    /// Available exclusive buffers in the existing pair, from zero through two.
    pub(crate) buffers: u8,
    pub(crate) render_id: u64,
    pub(crate) render: Work,
    pub(crate) display_id: u64,
    pub(crate) display: Work,
    pub(crate) pacer: Pacer,
}
/// Correlate preparation facts with an existing authenticated surface request and source intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) request_id: u64,
    pub(crate) intent: u64,
    pub(crate) current: u64,
    pub(crate) snapshot: NativePreparationSnapshot,
}
impl Record {
    /// Emit only closed, bounded facts when the existing Preparing request's snapshot changes.
    #[cfg(feature = "native-composition")]
    pub(crate) fn emit(self) {
        let s = self.snapshot;
        tracing::info!(operation="canvas_preparation_snapshot", schema=1, request_id=self.request_id,
            intent=self.intent, current=self.current, phase=%s.phase, flags=s.flags, scene_source=s.scene_source, buffers=s.buffers,
            render_id=s.render_id, render=%s.render, display_id=s.display_id,
            display=%s.display, pacer=%s.pacer);
    }
    /// Accept exact schema-one fields within the unchanged 512-byte child-output scratch.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"canvas_preparation_snapshot\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::canvas_preparation_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let request_id = number(field("request_id")?)?;
        let intent = number(field("intent")?)?;
        let current = number(field("current")?)?;
        let phase = Phase::parse(field("phase")?)?;
        let flags = u32::try_from(number(field("flags")?)?).ok()?;
        let scene_source = number(field("scene_source")?)?;
        let buffers = u8::try_from(number(field("buffers")?)?).ok()?;
        let render_id = number(field("render_id")?)?;
        let render = Work::parse(field("render")?)?;
        let display_id = number(field("display_id")?)?;
        let display = Work::parse(field("display")?)?;
        let pacer = Pacer::parse(field("pacer")?)?;
        if fields.next().is_some()
            || request_id == 0
            || flags & !FLAGS != 0
            || buffers > 2
            || (render_id == 0) != (render == Work::None)
            || (display_id == 0) != (display == Work::None)
        {
            return None;
        }
        Some(Self {
            request_id,
            intent,
            current,
            snapshot: NativePreparationSnapshot {
                phase,
                flags,
                scene_source,
                buffers,
                render_id,
                render,
                display_id,
                display,
                pacer,
            },
        })
    }
    /// Relay only validated fields under the actual child's retained run identity.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        let s = self.snapshot;
        tracing::info!(operation="desktop_child_canvas_preparation", run=?run,
            request_id=self.request_id, intent=self.intent, current=self.current,
            phase=%s.phase, flags=s.flags, scene_source=s.scene_source, buffers=s.buffers, render_id=s.render_id, render=%s.render,
            display_id=s.display_id, display=%s.display, pacer=%s.pacer,
            "Observed owned native canvas preparation snapshot");
    }
}
/// Refuse signs, padding, whitespace and numeric overflow at the passive wire boundary.
#[cfg(feature = "windows-native-host")]
fn number(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}
#[cfg(all(test, feature = "native-composition", feature = "windows-native-host"))]
#[path = "canvas_preparation_diagnostic/tests.rs"]
mod tests;
