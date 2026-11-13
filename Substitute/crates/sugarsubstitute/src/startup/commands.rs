// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one policy UI effect; close fences positive commands and reaches native teardown.
use std::error::Error;
use sugarsubstitute_contracts::startup::{LaunchId, MainIdentity};
use sugarsubstitute_desktop::DesktopStartupCommand;
/// Hold matching UI data only; native/profile/capture permission remains child-owned.
pub(super) struct StartupCommands {
    pending: Option<DesktopStartupCommand>,
    closing: bool,
}
impl StartupCommands {
    /// Start with no queued positive UI work or close request.
    pub(super) const fn new() -> Self {
        Self { pending: None, closing: false }
    }
    /// Retain the requested splash identity without replacing an outstanding effect.
    pub(super) fn retire_splash(&mut self, launch: LaunchId) -> Result<(), Box<dyn Error>> {
        self.admit()?;
        self.pending = Some(DesktopStartupCommand::RetireSplash(launch));
        Ok(())
    }
    /// Retain the exact application identity; this grants no native/profile permission.
    pub(super) fn reveal_main(&mut self, identity: MainIdentity) -> Result<(), Box<dyn Error>> {
        self.admit()?;
        self.pending = Some(DesktopStartupCommand::RevealMain(identity));
        Ok(())
    }
    fn admit(&self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Err(std::io::Error::other("Startup UI command admission closed").into());
        }
        if self.pending.is_some() {
            return Err(std::io::Error::other("Unconsumed startup UI command").into());
        }
        Ok(())
    }
    /// Retire queued positive UI data and request actual Desktop Session closing once.
    /// A content-only close cannot retire the native Main/Splash surface by itself.
    pub(super) fn close(&mut self) {
        if !self.closing {
            self.closing = true;
            self.pending = Some(DesktopStartupCommand::Close);
        }
    }
    /// Move one actual command without cloning requests or manufacturing observations.
    pub(super) fn take(&mut self) -> Option<DesktopStartupCommand> {
        self.pending.take()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
    };
    fn identity() -> Result<MainIdentity, Box<dyn Error>> {
        Ok(MainIdentity {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        })
    }
    #[test]
    fn unconsumed_policy_effect_is_rejected_without_replacing_original_command()
    -> Result<(), Box<dyn Error>> {
        let mut commands = StartupCommands::new();
        let current = identity()?;
        commands.retire_splash(current.launch)?;
        assert!(commands.reveal_main(current).is_err());
        assert!(
            matches!(commands.take(),Some(DesktopStartupCommand::RetireSplash(launch)) if launch==current.launch)
        );
        commands.reveal_main(current)?;
        assert!(
            matches!(commands.take(),Some(DesktopStartupCommand::RevealMain(actual)) if actual==current)
        );
        Ok(())
    }
    #[test]
    fn close_replaces_positive_ui_data_with_actual_native_close_and_fences_future_commands()
    -> Result<(), Box<dyn Error>> {
        let current = identity()?;
        for reveal in [false, true] {
            let mut commands = StartupCommands::new();
            if reveal {
                commands.reveal_main(current)?;
            } else {
                commands.retire_splash(current.launch)?;
            }
            commands.close();
            assert!(matches!(commands.take(), Some(DesktopStartupCommand::Close)));
            assert!(commands.retire_splash(current.launch).is_err());
            assert!(commands.reveal_main(current).is_err());
            assert!(commands.take().is_none());
        }
        Ok(())
    }
    #[test]
    fn repeated_close_does_not_requeue_or_claim_native_retirement() -> Result<(), Box<dyn Error>> {
        let mut commands = StartupCommands::new();
        commands.close();
        commands.close();
        assert!(matches!(commands.take(), Some(DesktopStartupCommand::Close)));
        commands.close();
        assert!(commands.take().is_none());
        Ok(())
    }
}
