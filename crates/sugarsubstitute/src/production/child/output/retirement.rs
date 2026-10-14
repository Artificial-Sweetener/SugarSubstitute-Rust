// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire only existing admitted output custody, without parsing or fresh pulls.
use super::*;
impl ProductionChild {
    /// Drain only the original closed pull and actual ended parser; issue no jobs or new pulls.
    pub(super) fn retire_output(&mut self) -> Result<()> {
        if self.output.job.is_some()
            || self.output.owner_mut().is_some_and(|owner| owner.worker_pending())
        {
            return Ok(());
        }
        let Some((header, _original)) = self.output.pending else {
            return Ok(());
        };
        let pending =
            self.output.owner_mut().ok_or("Original output owner missing")?.retiring_pull();
        if let Some((actual, _, false)) = pending {
            if actual != header {
                return Err("Retiring output header differs".into());
            }
            self.bootstrap.channel_mut().withdraw_prepared(header)?;
            self.output
                .owner_mut()
                .ok_or("Original output owner missing")?
                .finish_retiring_pull(header)?;
            self.output.pending = None;
            return Ok(());
        }
        if self.output.applied.is_none() && self.output.retiring_frame.is_none() {
            self.output.retiring_frame = self
                .output
                .owner_mut()
                .ok_or("Original output owner missing")?
                .take_unparsed_frame()?;
            if self.output.retiring_frame.is_none()
                && self.failed_frame.as_ref().is_some_and(|frame| {
                    FrameCodec::decode(frame.as_bytes())
                        .is_ok_and(|message| message.header == header)
                })
            {
                self.output.retiring_frame = self.failed_frame.take();
            }
            if self.output.retiring_frame.is_none()
                && self.bootstrap.channel_mut().received_header()? == Some(header)
            {
                self.output.retiring_frame = self.bootstrap.channel_mut().take_frame();
            }
        }
        if let Some(applied) = &self.output.applied {
            if applied.header != header {
                return Err("Applied output retirement header differs".into());
            }
        } else if let Some(frame) = &self.output.retiring_frame {
            let bytes = frame.as_bytes();
            let owner = match self.output.terminal.as_ref().map(|terminal| &terminal.outcome) {
                Some(JobOutcome::Success(owner)) => owner,
                _ => return Err("Original output source missing".into()),
            };
            if owner.validate_retiring_reply(bytes)? != header {
                return Err("Retiring output reply differs".into());
            }
        } else {
            return Ok(());
        }
        self.bootstrap.channel_mut().consume_reply(header)?;
        self.output
            .owner_mut()
            .ok_or("Original output owner missing")?
            .finish_retiring_pull(header)?;
        self.output.pending = None;
        self.output.applied = None;
        self.output.retiring_frame = None;
        tracing::info!(operation="closed_output_pull_retired",request=?header.request,run=?self.run,
            "Original output reply consumed after actual parser retirement");
        Ok(())
    }
}
