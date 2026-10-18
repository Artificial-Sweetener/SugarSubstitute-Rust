// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rebase actual native selection and viewport without treating tail-follow as focus.
use super::index::DocumentIndex;
use crate::DesktopError;
use std::rc::Rc;
/// Original native character positions and independently observed vertical scroll state.
#[derive(Clone, Copy, Default)]
pub(crate) struct Observation {
    pub(crate) start: i32,
    pub(crate) end: i32,
    pub(crate) first: i32,
    pub(crate) at_bottom: bool,
}
/// Replace external native effects; record identity and selection policy remain here.
pub(crate) trait TextPort {
    fn observe(&mut self) -> Result<Observation, DesktopError>;
    fn write(&mut self, document: &DocumentIndex) -> Result<(), DesktopError>;
    fn restore(
        &mut self,
        start: i32,
        end: i32,
        first: i32,
        follow: bool,
    ) -> Result<(), DesktopError>;
}
/// Keep the old document owner until all effects succeed; the caller already retains
/// incoming ownership before calling. Failure establishes no successful publication.
pub(crate) fn replace(
    current: Option<&DocumentIndex>,
    incoming: &DocumentIndex,
    port: &mut impl TextPort,
) -> Result<(), DesktopError> {
    let observation = if current.is_some() {
        port.observe()?
    } else {
        Observation { at_bottom: true, ..Default::default() }
    };
    if current.is_some_and(|old| {
        observation.start < 0
            || observation.end < observation.start
            || observation.end as u32 > old.units
            || observation.first < 0
            || observation.first as u32 > old.units
    }) {
        return Err(DesktopError::message(
            "console_native_observation",
            "native positions exceed original document",
        ));
    }
    let same = current.is_some_and(|old| old.resource == incoming.resource);
    let anchor = |position| current.filter(|_| same).and_then(|old| old.anchor(position));
    let start = incoming.position(anchor(observation.start));
    let end = incoming.position(anchor(observation.end));
    let first = incoming.position(anchor(observation.first));
    let follow = !same || observation.at_bottom && observation.start == observation.end;
    let (start, end) =
        if follow { (incoming.units as i32, incoming.units as i32) } else { (start, end) };
    port.write(incoming)?;
    port.restore(start, end, first, follow)
}
/// Retain both exact document claims on an uncertain partial native replacement.
/// A failed publication closes subsequent document admission until checked native teardown.
pub(crate) struct Documents {
    current: Option<Rc<DocumentIndex>>,
    pending: Option<Rc<DocumentIndex>>,
    failed: bool,
}
impl Documents {
    /// Adopt the already-admitted original document before native creation.
    pub(crate) fn new(incoming: DocumentIndex) -> Self {
        Self { current: None, pending: Some(Rc::new(incoming)), failed: false }
    }
    /// Borrow only the last complete native publication.
    pub(crate) fn current(&self) -> Option<&DocumentIndex> {
        self.current.as_deref()
    }
    /// Share the original immutable index and its existing admission claim with native painting.
    pub(crate) fn shared(&self) -> Option<Rc<DocumentIndex>> {
        self.current.clone()
    }
    /// Adopt a complete replacement before its first external text effect.
    pub(crate) fn stage(&mut self, incoming: DocumentIndex) -> Result<(), DesktopError> {
        if self.failed || self.pending.is_some() {
            return Err(DesktopError::message(
                "console_stage",
                "original document replacement remains unsettled",
            ));
        }
        if self.current.as_ref().is_some_and(|old| {
            old.resource == incoming.resource && old.revision > incoming.revision
        }) {
            return Err(DesktopError::message("console_stage", "source revision regressed"));
        }
        self.pending = Some(Rc::new(incoming));
        Ok(())
    }
    /// Preserve both owners and close admission on any uncertain native effect.
    pub(crate) fn publish(&mut self, port: &mut impl TextPort) -> Result<(), DesktopError> {
        if self.failed {
            return Err(DesktopError::message(
                "console_publish",
                "original native replacement failed",
            ));
        }
        let Some(incoming) = self.pending.as_ref() else {
            return Ok(());
        };
        if let Err(cause) = replace(self.current.as_deref(), incoming, port) {
            self.failed = true;
            return Err(cause);
        }
        self.current = self.pending.take();
        Ok(())
    }
    /// Whether an adopted replacement still needs native settlement.
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }
}
#[cfg(test)]
mod tests;
