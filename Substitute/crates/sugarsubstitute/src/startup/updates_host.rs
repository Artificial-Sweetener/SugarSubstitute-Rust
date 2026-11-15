// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the page's update requests through the startup host.
//!
//! The host answers from the window's update status and workflow queue, asks the coordinator
//! to check when the page asks to, starts the close when the page asks to restart now, and
//! retires the page's session with its navigation.
use super::*;

impl<B: StartupBackend> StartupHost<B> {
    /// Answer one page update request from the window's update status and workflow queue, ask
    /// the coordinator to check when the page asked to, and start the close when the page asked
    /// to restart now.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    fn answer_updates(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        message: sugarsubstitute_contracts::host_updates::UpdatesClientMessage,
        protocol: sugarsubstitute_contracts::host_updates::UpdatesProtocol,
        lease: sugarsubstitute_contracts::execution::MemoryLease,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_desktop::StartupWindowContent;
        let status = self.update_status();
        let queue = self.child.workflow_queue();
        let product = self.locale_messages().updates.product;
        let child = &mut self.child;
        let mut check = || child.request_update_check();
        let mut updater = host_updates::HostUpdates::new(
            product,
            status,
            queue,
            self.closing,
            &mut self.workflow_restart,
            &mut check,
        );
        let activity = host_updates::QueueActivity(queue);
        let request = updates_page::PageRequest { identity, message, protocol, lease, claim };
        self.updates.observe(request, &mut updater, &activity)?;
        if updater.close_now() {
            self.restart_to_apply()?;
        }
        Ok(())
    }

    /// Send the page a newer update state when the window's update status or workflow queue
    /// changed what it shows.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    pub(super) fn refresh_updates(&mut self) -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_desktop::StartupWindowContent;
        let status = self.update_status();
        let queue = self.child.workflow_queue();
        let product = self.locale_messages().updates.product;
        // Refreshing never asks for a check.
        let mut check = || Ok::<(), Box<dyn Error>>(());
        let updater = host_updates::HostUpdates::new(
            product,
            status,
            queue,
            self.closing,
            &mut self.workflow_restart,
            &mut check,
        );
        self.updates.refresh(&updater, &host_updates::QueueActivity(queue))
    }

    /// Consume the page's update requests and retire its session with its navigation; returns
    /// any other observation unchanged.
    #[cfg(all(windows, any(feature = "windows-native-host", test)))]
    pub(super) fn observe_updates(
        &mut self,
        observation: DesktopStartupObservation,
    ) -> Result<Option<DesktopStartupObservation>, Box<dyn Error>> {
        match observation {
            DesktopStartupObservation::UpdatesMessage(observed) => {
                let protocol = observed.protocol();
                let (identity, message, lease, claim) = observed.into_parts();
                self.answer_updates(identity, message, protocol, lease, claim)?;
                Ok(None)
            }
            DesktopStartupObservation::MainRetired(identity) => {
                self.updates.retire(identity);
                Ok(Some(DesktopStartupObservation::MainRetired(identity)))
            }
            DesktopStartupObservation::MainAbsent(identity) => {
                self.updates.retire(identity);
                Ok(Some(DesktopStartupObservation::MainAbsent(identity)))
            }
            DesktopStartupObservation::MainReloadRequested { identity, reason } => {
                self.updates.retire(identity);
                Ok(Some(DesktopStartupObservation::MainReloadRequested { identity, reason }))
            }
            other => Ok(Some(other)),
        }
    }
}
