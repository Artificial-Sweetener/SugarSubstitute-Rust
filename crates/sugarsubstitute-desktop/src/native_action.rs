// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queue native intent without lending callbacks content or scheduling handles.

#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeAction {
    /// Bounded execution metadata is available, including during owned close.
    Completions,
    /// Bounded actual native eligibility facts are ready for UI-thread dispatch.
    #[cfg(target_os = "windows")]
    Activity,
    /// Bounded main construction/receiver observations are available.
    #[cfg(target_os = "windows")]
    Main,
    /// Activate one registered native control.
    #[cfg(target_os = "windows")]
    Activate(u16),
    /// Activate a main caption control, independently of splash-local IDs.
    #[cfg(target_os = "windows")]
    MainControl(crate::MainIdentity, u16),
    /// Snapshot one current native recovery caption action without backend authority.
    #[cfg(target_os = "windows")]
    MainBackendRetry(crate::main_backend::RetryAction),
    /// Request application-owned close.
    #[cfg(target_os = "windows")]
    Close,
    /// Traverse only current visible native splash peers, preserving Shift direction.
    #[cfg(target_os = "windows")]
    FocusNext(bool),
    /// Repaint a retained splash source after its native focus cue changes.
    #[cfg(target_os = "windows")]
    FocusChanged,
    /// Navigate an owning page viewport without taking selector or editable-text key semantics.
    #[cfg(target_os = "windows")]
    ScrollPage(sugarsubstitute_ui::windows::scroll_port::ScrollKey),
}

#[cfg(target_os = "windows")]
impl NativeAction {
    /// Ignore queued caption intent from a retired or replaced native main lifetime.
    pub(crate) fn main_control(self, current: crate::MainIdentity) -> Option<u16> {
        match self {
            Self::MainControl(identity, id) if identity == current && (1..=3).contains(&id) => {
                Some(id)
            }
            _ => None,
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
        startup::LaunchId,
    };

    /// Even a close caption from a prior launch must not close the retry's new main.
    #[test]
    fn queued_main_caption_intent_requires_full_current_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let current = crate::MainIdentity {
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
        };
        let close = NativeAction::MainControl(current, 3);
        assert_eq!(close.main_control(current), Some(3));
        assert_eq!(NativeAction::MainControl(current, 4).main_control(current), None);
        let mut stale = current;
        stale.launch = LaunchId::new(2)?;
        assert_eq!(close.main_control(stale), None);
        stale = current;
        stale.navigation = Generation::new(2)?;
        assert_eq!(close.main_control(stale), None);
        stale = current;
        stale.run.generation = RunGeneration::new(2)?;
        assert_eq!(close.main_control(stale), None);
        stale = current;
        stale.run.owner.epoch = CoordinatorEpoch::new([3; 16])?;
        assert_eq!(close.main_control(stale), None);
        assert_eq!(NativeAction::Close.main_control(current), None);
        Ok(())
    }
}
