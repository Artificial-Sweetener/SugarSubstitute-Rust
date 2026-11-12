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
    /// Windows reported a UI color change; the loop rereads the system accent.
    #[cfg(target_os = "windows")]
    SystemAccent,
    /// Navigate an owning page viewport without taking selector or editable-text key semantics.
    #[cfg(target_os = "windows")]
    ScrollPage(sugarsubstitute_ui::windows::scroll_port::ScrollKey),
    /// An action from a form window's peers, kept apart from the loop's other surfaces.
    #[cfg(target_os = "windows")]
    Form(FormScope, FormAction),
}

/// Identity of one form window on an event loop it shares with other surfaces.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FormScope(pub(crate) u64);

/// What a form window's peers ask of that window.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FormAction {
    /// Activate one peer.
    Activate(u16),
    /// Close the form window.
    Close,
    /// Move focus, backwards with Shift.
    FocusNext(bool),
    /// A peer gained or lost focus.
    FocusChanged,
    /// Scroll the page.
    ScrollPage(sugarsubstitute_ui::windows::scroll_port::ScrollKey),
}

#[cfg(target_os = "windows")]
impl FormAction {
    /// The form action a peer callback's `action` means, if it is one.
    pub(crate) fn from_native(action: NativeAction) -> Option<Self> {
        Some(match action {
            NativeAction::Activate(id) => Self::Activate(id),
            NativeAction::Close => Self::Close,
            NativeAction::FocusNext(backward) => Self::FocusNext(backward),
            NativeAction::FocusChanged => Self::FocusChanged,
            NativeAction::ScrollPage(key) => Self::ScrollPage(key),
            _ => return None,
        })
    }
}

#[cfg(target_os = "windows")]
impl NativeAction {
    /// Ignore queued caption intent from a retired or replaced native main lifetime. The retry
    /// peer has its own action; window and update peers arrive here.
    pub(crate) fn main_control(self, current: crate::MainIdentity) -> Option<u16> {
        match self {
            Self::MainControl(identity, id)
                if identity == current
                    && ((1..=3).contains(&id) || crate::main_updates::owns(id)) =>
            {
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
        assert_eq!(NativeAction::MainControl(current, 5).main_control(current), Some(5));
        assert_eq!(NativeAction::MainControl(current, 10).main_control(current), None);
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

    /// A form window's peer actions keep their meaning under its scope; nothing else is a form
    /// action, so a hosted form can never close or retry the application.
    #[test]
    fn only_peer_actions_become_form_actions() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            FormAction::from_native(NativeAction::Activate(7)),
            Some(FormAction::Activate(7))
        );
        assert_eq!(FormAction::from_native(NativeAction::Close), Some(FormAction::Close));
        assert_eq!(
            FormAction::from_native(NativeAction::FocusNext(true)),
            Some(FormAction::FocusNext(true))
        );
        assert_eq!(
            FormAction::from_native(NativeAction::FocusChanged),
            Some(FormAction::FocusChanged)
        );
        assert_eq!(FormAction::from_native(NativeAction::Completions), None);
        assert_eq!(FormAction::from_native(NativeAction::SystemAccent), None);
        let identity = crate::MainIdentity {
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
        assert_eq!(FormAction::from_native(NativeAction::MainControl(identity, 3)), None);
        Ok(())
    }
}
