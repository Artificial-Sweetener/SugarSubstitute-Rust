// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native page publication against the real backend and its single execution mailbox.
use super::*;
use crate::{
    completion_wake::Wake,
    paused_clock::PausedClock,
    setup::presentation::{Action, Interaction},
};
use sugarsubstitute_application::setup_flow::{Page, PathField};
use sugarsubstitute_contracts::execution::ScopeSpec;
use sugarsubstitute_ui::{form::Role, text::PaintSpec};

/// Re-examine `poll` after each execution wake until it reports completion.
fn observe(wake: &Wake, mut poll: impl FnMut() -> Result<bool>) -> Result<()> {
    wake.until(|| poll().map(|done| done.then_some(())).transpose())
}
struct Fixture {
    controller: Controller,
    session: Session,
    wake: Arc<Wake>,
}
impl Fixture {
    /// Supply only the package/storage boundary; navigation, execution and rendering remain real.
    fn new() -> Result<Self> {
        let wake = Arc::new(Wake::default());
        let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
        controller.clock = PausedClock::new();
        let owner = ExecutionOwner::new(controller.clock.clone(), wake.clone())?;
        controller.client = Some(owner.client());
        controller.owner = Some(owner);
        controller.scope = Some(controller.create_scope(None)?);
        let mut session = Session::new(
            r"E:\用户\Substitute".into(),
            View {
                surface: PaintSpec {
                    width: 720,
                    height: 560,
                    scale: 1.0,
                    theme: sugarsubstitute_contracts::configuration::Theme::Dark,
                },
                now: Duration::ZERO,
                scroll: 0.0,
                interaction: Interaction::default(),
                text_revisions: [0; 5],
                accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
            },
        )?;
        session.start(&controller)?;
        Ok(Self { controller, session, wake })
    }
    /// Wait for a page matching current application input through real completion wakes.
    fn frame(&mut self, now: Duration) -> Result<()> {
        let wake = self.wake.clone();
        observe(&wake, || {
            self.session.advance(&mut self.controller, now)?;
            Ok(self.session.ready())
        })
    }
    /// Route the flow-owned primary intent after invalidating its previous frame.
    fn primary(&mut self) -> Result<()> {
        self.session.invalidate()?;
        if let Some(request) = self.session.navigation.flow.primary(Duration::ZERO)? {
            self.session.navigation.submit(&mut self.controller, request)?;
        }
        Ok(())
    }
    /// Drain render and backend custody before the original execution owner joins.
    fn finish(&mut self) -> Result<()> {
        self.session.close()?;
        self.controller.close()?;
        let wake = self.wake.clone();
        observe(&wake, || {
            self.session.advance(&mut self.controller, Duration::ZERO)?;
            assert!(!self.session.ready());
            if !self.session.settled() {
                return Ok(false);
            }
            self.controller.settled()
        })?;
        self.session.finish()?;
        self.controller.finish()?;
        Ok(())
    }
}

#[test]
fn native_session_applies_backend_navigation_before_accepting_rendered_pages() -> Result<()> {
    let mut f = Fixture::new()?;
    assert!(f.session.start(&f.controller).is_err());
    f.frame(Duration::ZERO)?;
    let original = f.session.prepared.as_ref().ok_or("Initial page absent")?.frame.identity();
    // An already admitted repaint must not win against the immediate backend Locale result.
    f.session.invalidate()?;
    f.session.advance(&mut f.controller, Duration::ZERO)?;
    f.primary()?;
    assert!(!f.session.ready());
    f.frame(Duration::ZERO)?;
    assert_eq!(f.session.navigation.flow.page(), Page::Location);
    let prepared = f.session.prepared.as_ref().ok_or("Location page absent")?;
    assert_ne!(prepared.frame.identity(), original);
    assert_eq!(prepared.frame.identity().surface.get(), 2);
    let field = prepared
        .layout
        .page_controls
        .iter()
        .find(|control| control.id == Action::Path(PathField::Installation).id())
        .ok_or("Native installation field absent")?;
    assert_eq!(field.role, Role::TextField(r"E:\用户\Substitute".into()));
    assert!(f.controller.output.is_empty());
    assert_eq!(f.session.view.scroll, 0.0);
    assert!(!f.session.settled());
    assert!(f.session.finish().is_err());
    f.finish()
}

#[test]
fn native_session_wait_feedback_does_not_cancel_backend_and_close_drains_actual_rendering()
-> Result<()> {
    let mut f = Fixture::new()?;
    f.frame(Duration::ZERO)?;
    f.primary()?;
    f.frame(Duration::ZERO)?;
    f.primary()?; // Package inspection remains pending at the unmaterialized package boundary.
    f.frame(Duration::ZERO)?;
    let before = f.session.prepared.as_ref().ok_or("Busy page absent")?.frame.clone();
    let pending = f.controller.waiting.as_ref().ok_or("Inspection missing")?.id.clone();
    f.session.advance(&mut f.controller, Duration::from_secs(120))?;
    assert!(!f.session.ready());
    f.frame(Duration::from_secs(120))?;
    assert!(f.session.waiting && f.session.navigation.flow.busy());
    assert_eq!(f.controller.waiting.as_ref().ok_or("Inspection was cancelled")?.id, pending);
    assert_ne!(
        before.pixels(),
        f.session.prepared.as_ref().ok_or("Wait page absent")?.frame.pixels()
    );
    f.session.invalidate()?;
    f.session.advance(&mut f.controller, Duration::from_secs(121))?;
    f.session.close()?;
    assert!(!f.session.ready());
    assert!(f.session.finish().is_err());
    f.finish()?;
    assert!(f.controller.waiting.is_none() && f.controller.owner.is_none());
    Ok(())
}

#[test]
fn native_session_close_retires_render_when_root_failure_consumed_its_notification() -> Result<()> {
    let mut f = Fixture::new()?;
    f.frame(Duration::ZERO)?;
    f.session.invalidate()?;
    f.session.advance(&mut f.controller, Duration::ZERO)?;
    // Model failure after the root's drain but before the batch reaches the renderer.
    let wake = f.wake.clone();
    observe(&wake, || {
        let advanced = f.controller.advance_events()?;
        assert!(advanced.events.is_empty());
        Ok(!advanced.completions.notifications.is_empty())
    })?;
    assert!(!f.session.ready());
    f.finish()?;
    assert!(f.session.settled());
    assert!(f.controller.owner.is_none());
    Ok(())
}

/// Rapid edits and clicks remain admitted during hover/text repaint; older document events lose.
#[test]
fn native_input_keeps_latest_edits_during_repaint_and_rejects_busy_edits() -> Result<()> {
    use sugarsubstitute_desktop::installer::native::Event as NativeEvent;
    use sugarsubstitute_ui::form::Input;
    let mut f = Fixture::new()?;
    f.frame(Duration::ZERO)?;
    f.session.input(
        &mut f.controller,
        NativeEvent::Interaction {
            hovered: Some(Action::Primary.id()),
            pressed: None,
            focused: None,
        },
    )?;
    assert!(!f.session.ready());
    f.session
        .input(&mut f.controller, NativeEvent::Control(Action::Primary.id(), Input::Invoke))?;
    f.frame(Duration::ZERO)?;
    assert_eq!(f.session.navigation.flow.page(), Page::Location);
    let id = Action::Path(PathField::Installation).id();
    for (revision, text) in [(1, "First"), (2, "Latest🙂"), (1, "Stale")] {
        f.session.input(
            &mut f.controller,
            NativeEvent::Control(id, Input::Text { value: Some(text.into()), revision }),
        )?;
    }
    assert_eq!(f.session.navigation.flow.choices().install_root, "Latest🙂");
    assert_eq!(f.session.view.text_revisions[0], 2);
    f.frame(Duration::ZERO)?;
    f.session
        .input(&mut f.controller, NativeEvent::Control(Action::Primary.id(), Input::Invoke))?;
    f.session.input(
        &mut f.controller,
        NativeEvent::Control(id, Input::Text { value: Some("Too late".into()), revision: 3 }),
    )?;
    assert_eq!(f.session.navigation.flow.choices().install_root, "Latest🙂");
    f.finish()
}

/// Modal results preserve cancellation, reject stale/foreign replies and report recoverable errors.
#[test]
fn native_picker_correlation_preserves_current_user_text() -> Result<()> {
    use sugarsubstitute_desktop::installer::native::Event as NativeEvent;
    use sugarsubstitute_ui::form::Input;
    let mut f = Fixture::new()?;
    f.frame(Duration::ZERO)?;
    f.primary()?;
    f.frame(Duration::ZERO)?;
    let field = Action::Path(PathField::Installation).id();
    let browse = Action::Browse(PathField::Installation).id();
    f.session.input(&mut f.controller, NativeEvent::Control(browse, Input::Invoke))?;
    let first = f.session.picker().ok_or("Native picker absent")?;
    assert!(f.session.picker().is_none());
    f.session.input(
        &mut f.controller,
        NativeEvent::Control(field, Input::Text { value: Some("User edit".into()), revision: 1 }),
    )?;
    f.session
        .input(&mut f.controller, NativeEvent::Picked(first, Ok(Some("Stale selection".into()))))?;
    assert_eq!(f.session.navigation.flow.choices().install_root, "User edit");
    f.frame(Duration::ZERO)?;
    f.session.input(&mut f.controller, NativeEvent::Control(browse, Input::Invoke))?;
    let second = f.session.picker().ok_or("Second picker absent")?;
    f.session.input(
        &mut f.controller,
        NativeEvent::Picked(first, Ok(Some("Foreign selection".into()))),
    )?;
    f.session.input(&mut f.controller, NativeEvent::Picked(second, Ok(None)))?;
    assert_eq!(f.session.navigation.flow.choices().install_root, "User edit");
    f.frame(Duration::ZERO)?;
    f.session.input(&mut f.controller, NativeEvent::Control(browse, Input::Invoke))?;
    let third = f.session.picker().ok_or("Third picker absent")?;
    f.session
        .input(&mut f.controller, NativeEvent::Picked(third, Err("Picker unavailable".into())))?;
    assert!(f.session.navigation.flow.failure().is_some());
    assert_eq!(f.session.navigation.flow.choices().install_root, "User edit");
    f.frame(Duration::ZERO)?;
    f.session.input(&mut f.controller, NativeEvent::Control(browse, Input::Invoke))?;
    let fourth = f.session.picker().ok_or("Fourth picker absent")?;
    f.session
        .input(&mut f.controller, NativeEvent::Picked(fourth, Ok(Some("Selected🙂".into()))))?;
    assert_eq!(f.session.navigation.flow.choices().install_root, "Selected🙂");
    f.finish()
}

/// Normal system mode changes retire old paint; repeated observations keep current pixels valid.
#[test]
fn system_theme_change_invalidates_only_changed_prepared_mode() -> Result<()> {
    use sugarsubstitute_contracts::configuration::Theme;
    use sugarsubstitute_desktop::installer::native::Event as NativeEvent;
    let mut fixture = Fixture::new()?;
    fixture.frame(Duration::ZERO)?;
    for theme in [Theme::Light, Theme::Dark] {
        fixture.session.input(&mut fixture.controller, NativeEvent::SystemTheme(theme))?;
        assert_eq!(fixture.session.view.surface.theme, theme);
        assert!(!fixture.session.ready());
        fixture.frame(Duration::ZERO)?;
        fixture.session.input(&mut fixture.controller, NativeEvent::SystemTheme(theme))?;
        assert!(fixture.session.ready());
    }
    fixture.finish()
}

/// System appearance reaches the retained PaintSpec before execution or native page admission.
#[test]
fn initial_system_theme_updates_inert_controller_paint_spec() -> Result<()> {
    use sugarsubstitute_contracts::configuration::Theme;
    use sugarsubstitute_desktop::installer::native::Event as NativeEvent;
    let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
    let mut session = Session::new(
        "Original folder".into(),
        View {
            surface: PaintSpec { width: 1180, height: 760, scale: 1.0, theme: Theme::Dark },
            accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
            now: Duration::ZERO,
            scroll: 0.0,
            interaction: Interaction::default(),
            text_revisions: [0; 5],
        },
    )?;
    for theme in [Theme::Light, Theme::Dark] {
        session.input(&mut controller, NativeEvent::SystemTheme(theme))?;
        assert_eq!(session.view.surface.theme, theme);
        assert!(!session.ready());
        assert!(controller.owner.is_none());
    }
    Ok(())
}

#[test]
fn provisioning_scopes_created_after_the_native_session_follow_it_in_identity_order() -> Result<()>
{
    // Window start created the root and the session scope; the first page that needs the
    // provisioning worker then creates the worker scope and the worker's process-family scope.
    let mut f = Fixture::new()?;
    let root = f.controller.scope.as_ref().ok_or("Installer root scope absent")?;
    let worker = f.controller.create_scope(Some(root))?;
    let client = f.controller.client.clone().ok_or("Installer execution absent")?;
    let process = client.create_scope(
        ScopeSpec { id: f.controller.scopes.allocate()?, deadline: None },
        Some(&worker),
    )?;
    drop((process, worker));
    f.finish()
}
