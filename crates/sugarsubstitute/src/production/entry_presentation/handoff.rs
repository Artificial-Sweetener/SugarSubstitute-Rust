// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sequence complete first presentation and authenticated same-run native surface handoff.
use super::*;
impl<C: EntryActivation> EntryPresentation<C> {
    /// Start entry once, only from actual successful native submission of the accepted complete frame.
    pub(super) fn presented(&mut self, frame: FrameIdentity) -> Result<(), Box<dyn Error>> {
        if self.first_presented || self.closing {
            return Ok(());
        }
        if self.first_prepared != Some(frame) {
            return Err("Entry presentation identity mismatch".into());
        }
        self.first_presented = true;
        self.entry_mut()?.begin_entry()
    }
    /// Retain an actual covered-child request before asking the native owner to hide Root.
    pub(super) fn observe_handoff(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.first_presented || self.closing {
            return Ok(());
        }
        let state = self.entry_ref()?.entry_splash();
        if self.hidden {
            if let Some(identity) = state.handoff {
                self.handoff = Some((identity, state.presentation));
                self.release_child(identity)?;
            }
            return Ok(());
        }
        if let Some(identity) = state.handoff {
            if self.handoff.is_some_and(|original| original != (identity, state.presentation)) {
                return Err("Entry splash handoff identity changed".into());
            }
            self.handoff = Some((identity, state.presentation));
        }
        Ok(())
    }
    /// Authorize the child only after the native adapter acknowledges Root's actual hide.
    pub(super) fn complete_handoff(&mut self) -> Result<(), Box<dyn Error>> {
        if self.hidden || self.closing {
            return Ok(());
        }
        let (identity, _) = self.handoff.ok_or("Entry splash has no authenticated handoff")?;
        self.hidden = true;
        self.visible = false;
        if let Some(render) = &mut self.preparation {
            render.request_retirement()?;
            if render.poll_retirement()? {
                render.finish_retirement()?;
            }
        }
        self.release_child(identity)
    }
    /// Flush the retained authenticated reply on the next native turn, including while hidden.
    fn release_child(&mut self, identity: MainIdentity) -> Result<(), Box<dyn Error>> {
        self.entry_mut()?.complete_entry_splash_handoff(identity)?;
        self.continuation = Some(Instant::now());
        Ok(())
    }
}
