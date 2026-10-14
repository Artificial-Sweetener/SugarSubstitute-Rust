// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reborrow one canonical fragment and one exact prepared request through transport contention.
use super::*;
use sugarsubstitute_supervision::lifecycle_protocol::{
    FolderFragment, MAX_FOLDER_FRAGMENT_UNITS, RawWindowsUnits, WireMessage, WirePayload,
};
impl ProfileTransaction {
    /// Admit at most one request. False retains the same prepared header and original cutoff.
    /// Native writes remain with the actual channel owner; no UI callback copies folder bytes.
    /// Completed and closed transactions remain inert while their original custody is retained.
    /// # Errors
    /// Preserves channel/codec causes and rejects expired or mismatched active transactions.
    pub(in super::super) fn activity(
        &mut self,
        channel: &mut ChildChannel,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if !self.activity_required()? {
            return Ok(false);
        }
        let binding = channel.binding().ok_or(ProfileError::ChannelClosed)?;
        if binding.run() != self.identity().run
            || channel.peer().is_none_or(|peer| peer.run() != self.identity().run)
        {
            return Err(ProfileError::Identity.into());
        }
        if self.pending.is_some_and(|pending| pending.admitted) {
            return Ok(false);
        }
        if self.pending.is_none() {
            let header = channel.prepare_request(self.progress.original)?;
            self.pending = Some(Pending { header, admitted: false });
        }
        let header = self.pending.ok_or(ProfileError::Order)?.header;
        let payload = if self.progress.phase == Phase::Browser {
            WirePayload::ControllerPrepared {
                identity: self.identity(),
                browser_process_id: self.browser.ok_or(ProfileError::Order)?,
            }
        } else {
            let units = match self.progress.phase {
                Phase::Requested => &self.requested.units[..],
                Phase::Environment => {
                    self.environment.as_ref().ok_or(ProfileError::Order)?.as_utf16()
                }
                _ => return Err(ProfileError::Order.into()),
            };
            let end = self.progress.next_offset();
            let fragment = FolderFragment::new(
                self.identity(),
                self.progress.total as u32,
                self.progress.offset as u32,
                RawWindowsUnits::from_units(
                    units.get(self.progress.offset..end).ok_or(ProfileError::Folder)?,
                )?,
            )?;
            if fragment.units().unit_len() > MAX_FOLDER_FRAGMENT_UNITS {
                return Err(ProfileError::Folder.into());
            }
            if self.progress.phase == Phase::Requested {
                WirePayload::RequestedProfileFolder(fragment)
            } else {
                WirePayload::EnvironmentPrepared(fragment)
            }
        };
        let accepted =
            channel.try_send(&WireMessage { header, payload }, self.progress.original)?;
        if accepted {
            self.pending.as_mut().ok_or(ProfileError::Order)?.admitted = true;
        }
        Ok(accepted)
    }
}
