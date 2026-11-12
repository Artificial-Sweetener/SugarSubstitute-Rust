// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Place, present and release the form window's prepared pages and native peers.
use super::*;

impl FormWindow {
    /// Centre the window over its owner, or on the cursor's monitor without one.
    pub(super) fn place(&self) -> Result {
        let presenter = self.presenter.as_ref().ok_or("Native form presenter absent")?;
        match self.owner {
            Some(FormOwner(owner)) => presenter.center_over(owner)?,
            None => presenter.recenter()?,
        }
        Ok(())
    }

    pub(super) fn present(&mut self, lifetime: &mut impl Lifetime) -> Result {
        if let Some(page) = lifetime.page()
            && Some(page.frame.shape()) == self.shape
            && Some(page.frame.identity()) != self.identity
        {
            let next = Projection::new(lifetime.inventory(), &page, self.close)?;
            self.scroll.present(page.frame, page.viewport, page.content_height, page.scroll)?;
            if self.controls.is_none() {
                let fields = lifetime
                    .inventory()
                    .iter()
                    .filter(|item| {
                        matches!(item.role, sugarsubstitute_ui::form::Role::TextField(_))
                    })
                    .count();
                self.storage =
                    Some(page.frame.claim_storage(2 * 1024 * 1024 * (fields as u64 + 1))?);
                let mut hidden = next.specifications.clone();
                for spec in &mut hidden {
                    spec.control.visible = false;
                }
                let mut owner = Controls::new_form(
                    self.window.clone(),
                    self.proxy.clone(),
                    &hidden,
                    self.scope,
                )?;
                owner.initialize()?;
                self.controls = Some(owner);
            }
            let owner = self.controls.as_mut().ok_or("Native form owner absent")?;
            owner.update(&next.specifications)?;
            owner.clip(&next.clips)?;
            owner.present_fields(page.frame, self.window.theme() == Theme::Dark)?;
            page.frame
                .fund_storage(self.storage.as_mut().ok_or("Native control storage absent")?)?;
            let painter = self.presenter.as_mut().ok_or("Native form presenter absent")?;
            painter.present(page.frame, None)?;
            self.identity = Some(page.frame.identity());
            self.projection = Some(next);
            if !self.revealed {
                self.window.set_visible(self.options_visible);
                if self.options_visible {
                    self.window.set_focus();
                }
                self.revealed = true;
            }
            painter.expose_prepared()?;
            let projection = self.projection.as_ref().ok_or("Native page projection absent")?;
            let observations = super::super::next_observations(owner, projection)?;
            for (id, input) in observations {
                lifetime.input(Event::Control(id, input))?;
            }
            lifetime.presented()?;
        }
        if let Some(prepared) = lifetime.console() {
            if self.owner.is_some() {
                return Err("A form opened over the main window cannot show a console".into());
            }
            if let Some(owner) = &mut self.controls {
                owner.present_console(
                    prepared.frame,
                    Some(prepared.view),
                    prepared.focus_return.map(|id| id.get()),
                )?;
            }
        }
        self.scroll.paint()?;
        #[cfg(any(test, feature = "boundary-fixtures"))]
        if let Some(frame) = lifetime
            .page()
            .filter(|page| Some(page.frame.identity()) == self.identity)
            .map(|page| page.frame.clone())
            && let Some(observer) = lifetime.pixel_capture()
        {
            observer.surface(&frame);
            let projection = self.projection.as_ref().ok_or("Native capture projection absent")?;
            self.controls
                .as_ref()
                .ok_or("Native capture controls absent")?
                .capture(&projection.clips, observer)?;
            self.scroll.capture(observer)?;
            observer.finished();
        }
        Ok(())
    }

    /// Release peers and buffers once the lifetime lets the window go and no picker is open.
    pub(super) fn release(&mut self, lifetime: &mut impl Lifetime) -> Result<bool> {
        if !lifetime.window_releasable()? || self.picker.busy() {
            return Ok(false);
        }
        if let Some(owner) = &mut self.controls {
            owner.shutdown()?;
            if !owner.retired() {
                return Ok(false);
            }
        }
        self.controls = None;
        self.projection = None;
        self.storage = None;
        if let Some(owner) = &mut self.presenter {
            owner.shutdown()?;
        }
        Ok(true)
    }
}
