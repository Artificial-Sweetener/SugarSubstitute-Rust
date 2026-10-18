// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the complete entry splash before unrelated startup and its authenticated Desktop handoff.
use super::{
    entry_runner::{EntryActivation, EntryRunner},
    startup::view::StartupView,
};
use crate::preparation::PreparationDriver;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    configuration::{Presentation, Theme},
    execution::CompletionWake,
    presentation::{FrameIdentity, FrameShape, PreparedFrame},
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{
    AccessibleControl, DesktopOptions, HeadlessLifetime, HitTarget, KeyAction, Pointer,
    PointerAction, WindowContent,
};

#[path = "entry_presentation/handoff.rs"]
mod handoff;
#[path = "entry_presentation/lifetime.rs"]
mod lifetime;
#[cfg(test)]
#[path = "entry_presentation/tests.rs"]
mod tests;

/// Compose presentation with the same Root execution, admission, and captured child owners.
struct EntryPresentation<C: EntryActivation> {
    entry: Option<Box<EntryRunner<C>>>,
    preparation: Option<PreparationDriver>,
    view: StartupView,
    first_prepared: Option<FrameIdentity>,
    first_presented: bool,
    handoff: Option<(MainIdentity, Option<Presentation>)>,
    continuation: Option<Instant>,
    hidden: bool,
    visible: bool,
    closing: bool,
}

/// Run one complete Root splash and retain its lifetime until actual owned shutdown finishes.
pub(super) fn run<C: EntryActivation + 'static>(
    entry: Box<EntryRunner<C>>,
) -> Result<(), Box<dyn Error>> {
    let content = EntryPresentation::new(entry)?;
    let result = sugarsubstitute_desktop::run(
        DesktopOptions {
            title: sugarsubstitute::APPLICATION_NAME.into(),
            logical_width: 558,
            logical_height: 558,
            dark: true,
            duration: None,
            audit_frames: false,
            audit_timings: false,
        },
        content,
    );
    match result {
        Ok(_) => Ok(()),
        Err(cause) => {
            Err(report_failure(Box::new(cause), sugarsubstitute_desktop::show_startup_failure))
        }
    }
}

/// Present only this user-facing Root's failed lifetime, retaining its exact original error.
fn report_failure<E: Error>(
    cause: Box<dyn Error>,
    show: impl FnOnce(&str, &str, std::fmt::Arguments<'_>) -> Result<(), E>,
) -> Box<dyn Error> {
    if let Err(secondary) = show(
        sugarsubstitute::APPLICATION_NAME,
        sugarsubstitute_contracts::localization::messages("en").report.startup_failed,
        format_args!("{:?}", super::entry_runner::FailureDisplay(cause.as_ref())),
    ) {
        tracing::error!(operation="startup_failure_presentation",cause=?secondary,
            "Native startup failure presentation failed while its original application cause remains retained");
    }
    cause
}

impl<C: EntryActivation + 'static> WindowContent for EntryPresentation<C> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.preparation = Some(self.entry_mut()?.start_presentation(wake)?);
        self.preparation
            .as_mut()
            .ok_or("Entry splash preparation missing")?
            .set_visible(self.visible)
    }
    fn request_frame(
        &mut self,
        shape: FrameShape,
        elapsed: Duration,
    ) -> Result<(), Box<dyn Error>> {
        self.view.elapsed = elapsed;
        self.view.animation.tick(elapsed);
        let phase = if self.entry_ref()?.entry_setup_started() {
            sugarsubstitute_application::startup::StartupPhase::PreparingConnection
        } else {
            sugarsubstitute_application::startup::StartupPhase::SplashFirstFrame
        };
        self.preparation.as_mut().ok_or("Entry splash preparation missing")?.request(
            shape,
            self.view.appearance,
            self.view.animation.frame(elapsed),
            self.view.progress(phase),
            None,
        )
    }
    fn poll_frame(&mut self, max: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        self.continuation = None;
        let render = self.preparation.as_mut().ok_or("Entry splash preparation missing")?;
        let frame = self
            .entry
            .as_mut()
            .ok_or("Entry lifetime already transferred")?
            .consume_presentation(render, max)?;
        if let Some(frame) = &frame {
            if !self.first_presented {
                self.first_prepared = Some(frame.identity());
            }
            self.view.publish_controls();
        }
        self.observe_handoff()?;
        Ok(frame)
    }
    fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>> {
        self.visible = visible && !self.closing && !self.hidden;
        if let Some(render) = &mut self.preparation {
            render.set_visible(self.visible)?;
        }
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.close_owned()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.poll_owned_shutdown()
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.entry_mut()?.finish_shutdown()?;
        if let Some(render) = &mut self.preparation {
            render.finish_retirement()?;
        }
        Ok(())
    }
    fn hit_test(&self, pointer: Pointer) -> HitTarget {
        self.view.hit_test(pointer)
    }
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool {
        let changed = self.view.pointer(action, pointer);
        self.invalidate(changed)
    }
    fn key(&mut self, action: KeyAction) -> bool {
        self.view.key(action)
    }
    fn animating(&self) -> bool {
        !self.hidden && !self.closing && self.view.motion()
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        self.view.controls()
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::ConsoleView<'_>> {
        Some(self.view.console(None))
    }
    fn activate(&mut self, id: u16) -> bool {
        let changed = self.view.activate(id);
        self.invalidate(changed)
    }
    fn frame_presented(&mut self, frame: FrameIdentity) -> Result<(), Box<dyn Error>> {
        self.presented(frame)
    }
    fn presentation_visible(&self) -> bool {
        !self.hidden && self.handoff.is_none()
    }
    fn presentation_hidden(&mut self) -> Result<(), Box<dyn Error>> {
        self.complete_handoff()
    }
    fn close_requested(&self) -> bool {
        self.closing || self.entry.as_ref().is_none_or(|entry| entry.closing())
    }
    fn activity_deadline(&self) -> Option<Instant> {
        self.continuation
            .into_iter()
            .chain(self.entry.as_ref().and_then(|entry| entry.next_deadline()))
            .min()
    }
    fn retain_failure(&mut self, cause: sugarsubstitute_desktop::DesktopError) -> Box<dyn Error> {
        self.transfer_failure(Box::new(cause))
    }
}

impl<C: EntryActivation> EntryPresentation<C> {
    /// Transfer the exact error and all entry/render custody after native teardown attempts finish.
    fn transfer_failure(&mut self, cause: Box<dyn Error>) -> Box<dyn Error>
    where
        C: 'static,
    {
        self.closing = true;
        self.visible = false;
        self.continuation = None;
        match self.entry.take() {
            Some(entry) => {
                super::entry_runner::retain_failure(cause, entry, self.preparation.take())
            }
            None => cause,
        }
    }
    /// Construct only inert localized view state before the native adapter supplies its wake.
    fn new(entry: Box<EntryRunner<C>>) -> Result<Self, Box<dyn Error>> {
        let appearance = Presentation::new("en", Theme::Dark, false)?;
        Ok(Self {
            entry: Some(entry),
            preparation: None,
            view: StartupView::new(appearance),
            first_prepared: None,
            first_presented: false,
            handoff: None,
            continuation: None,
            hidden: false,
            visible: false,
            closing: false,
        })
    }
    /// Invalidate the single renderer only for actual view state changes.
    fn invalidate(&mut self, changed: bool) -> bool {
        if changed && let Some(render) = &mut self.preparation {
            render.invalidate();
        }
        changed
    }
    /// Borrow the live entry owner before its single post-native error transfer.
    fn entry_ref(&self) -> Result<&EntryRunner<C>, Box<dyn Error>> {
        self.entry.as_deref().ok_or_else(|| "Entry lifetime already transferred".into())
    }
    /// Mutably borrow the live entry owner before its single post-native error transfer.
    fn entry_mut(&mut self) -> Result<&mut EntryRunner<C>, Box<dyn Error>> {
        self.entry.as_deref_mut().ok_or_else(|| "Entry lifetime already transferred".into())
    }
}
