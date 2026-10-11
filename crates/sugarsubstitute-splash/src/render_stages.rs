// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose the same bounded paint stages for synchronous qualification and admitted jobs.

use super::{RenderError, RenderFrame, SplashRenderer};
use crate::{
    Control, Theme,
    caption_icons::{CaptionCursor, CaptionIcon},
    character::CharacterRender,
    chrome::{ConsoleCursor, ControlsCursor},
    progress::ProgressCursor,
    text::{StatusPaint, StatusText},
};
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{execution::MemoryLease, localization::messages};
use tiny_skia::{IntSize, Pixmap};

enum Stage {
    Clear,
    Character,
    Console,
    Fonts,
    StatusPrepare,
    Status,
    ProgressPrepare,
    Progress,
    Controls,
    Caption,
    Done,
}
/// Own target storage and only the currently live paint owner's scratch.
pub(super) struct RenderStages {
    frame: RenderFrame,
    bytes: Vec<u8>,
    target: Option<Pixmap>,
    stage: Stage,
    character: Option<CharacterRender>,
    console: Option<ConsoleCursor>,
    progress: Option<ProgressCursor>,
    caption: Option<(CaptionCursor, bool)>,
    status: Option<StatusPaint>,
    controls: Option<ControlsCursor>,
}
impl RenderStages {
    /// Validate public inputs and reserve output storage without zeroing it.
    pub(super) fn new(frame: RenderFrame) -> Result<Self, RenderError> {
        if frame.width == 0
            || frame.height == 0
            || frame.width > 4096
            || frame.height > 4096
            || !frame.scale.is_finite()
            || !(0.5..=4.0).contains(&frame.scale)
            || !frame.feedback.is_valid()
        {
            return Err(RenderError::InvalidFrame);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(frame.width as usize * frame.height as usize * 4)
            .map_err(RenderError::Buffer)?;
        Ok(Self {
            frame,
            bytes,
            target: None,
            stage: Stage::Clear,
            character: None,
            console: None,
            progress: None,
            caption: None,
            status: None,
            controls: None,
        })
    }
    /// Report the actual reserved output capacity before claiming live buffer ownership.
    pub(super) fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
    /// Clear <=32768 pixels, advance one owner slice, or execute bounded status/native input.
    ///
    /// Each caption control covers <=23680 pixels and one glyph <=23552 pixels. Status input
    /// is the locale owner's fixed label; native font calls are indivisible and their
    /// actual return establishes cancellation latency, with a four-MiB glyph raster cap.
    pub(super) fn step(&mut self, renderer: &mut SplashRenderer) -> Result<bool, RenderError> {
        self.step_admitted(renderer, None)
    }
    /// Let the admitted text cursor claim its own metadata/texture storage from the real lease.
    pub(super) fn step_admitted(
        &mut self,
        renderer: &mut SplashRenderer,
        lease: Option<MemoryLease>,
    ) -> Result<bool, RenderError> {
        if matches!(self.stage, Stage::Clear) {
            let length = self.frame.width as usize * self.frame.height as usize * 4;
            let end = (self.bytes.len() + 32768 * 4).min(length);
            self.bytes.resize(end, 0);
            if end == length {
                let size = IntSize::from_wh(self.frame.width, self.frame.height)
                    .ok_or(RenderError::InvalidFrame)?;
                self.target = Some(
                    Pixmap::from_vec(std::mem::take(&mut self.bytes), size)
                        .ok_or(RenderError::Allocation)?,
                );
                let mut bounds = self.frame.appearance.mascot();
                bounds.x *= self.frame.scale;
                bounds.y *= self.frame.scale;
                bounds.width *= self.frame.scale;
                bounds.height *= self.frame.scale;
                self.character = Some(
                    CharacterRender::new(
                        self.frame.width,
                        self.frame.height,
                        bounds,
                        self.frame.character,
                    )
                    .map_err(RenderError::Character)?,
                );
                self.stage = Stage::Character;
            }
            return Ok(false);
        }
        let target = self.target.as_mut().ok_or(RenderError::InvalidFrame)?;
        let frame = &self.frame;
        match self.stage {
            Stage::Character => {
                let cursor = self.character.as_mut().ok_or(RenderError::InvalidFrame)?;
                if cursor.step(&mut renderer.characters, target).map_err(RenderError::Character)? {
                    self.character = None;
                    if frame.appearance.expanded {
                        self.console = Some(ConsoleCursor::new(
                            frame.width,
                            frame.height,
                            frame.appearance.theme,
                            frame.scale as f32,
                        )?);
                        self.stage = Stage::Console;
                    } else if frame.status {
                        self.stage = Stage::Fonts;
                    } else {
                        self.begin_progress()?;
                    }
                }
            }
            Stage::Console => {
                let cursor = self.console.as_mut().ok_or(RenderError::InvalidFrame)?;
                if cursor.step(target)? {
                    self.console = None;
                    self.begin_progress()?;
                }
            }
            Stage::Fonts => {
                if renderer.text.is_none() {
                    renderer.text =
                        Some(StatusText::new(&renderer.locale).map_err(RenderError::Font)?);
                }
                self.stage = Stage::StatusPrepare;
            }
            Stage::StatusPrepare => {
                let text = renderer.text.as_mut().ok_or(RenderError::InvalidFrame)?;
                let message = messages(&renderer.locale).loading;
                let cursor = match lease {
                    Some(lease) => text.prepare(
                        message,
                        frame.appearance.theme,
                        frame.scale as f32,
                        frame.width,
                        frame.height,
                        lease,
                    ),
                    None => text.prepare_offline(
                        message,
                        frame.appearance.theme,
                        frame.scale as f32,
                        frame.width,
                        frame.height,
                    ),
                }
                .map_err(RenderError::Font)?;
                self.status = Some(cursor);
                self.stage = Stage::Status;
            }
            Stage::Status => {
                let cursor = self.status.as_mut().ok_or(RenderError::InvalidFrame)?;
                let max = NonZeroUsize::new(32768).ok_or(RenderError::InvalidFrame)?;
                if cursor.step(target, max).map_err(RenderError::Font)? {
                    self.status = None;
                    self.stage = Stage::ProgressPrepare;
                }
            }
            Stage::ProgressPrepare => self.begin_progress()?,
            Stage::Progress => {
                let cursor = self.progress.as_mut().ok_or(RenderError::InvalidFrame)?;
                if cursor.step(target)? {
                    self.progress = None;
                    self.controls = Some(ControlsCursor::new(frame.appearance, frame.scale as f32));
                    self.stage = Stage::Controls;
                }
            }
            Stage::Controls => {
                let cursor = self.controls.as_mut().ok_or(RenderError::InvalidFrame)?;
                if !cursor.step(target) {
                    return Ok(false);
                }
                self.controls = None;
                let tint = if frame.appearance.theme == Theme::Dark { [255; 3] } else { [0; 3] };
                self.caption = Some((
                    CaptionCursor::new(frame.scale, CaptionIcon::Console, tint)
                        .map_err(RenderError::Icon)?,
                    true,
                ));
                self.stage = Stage::Caption;
            }
            Stage::Caption => {
                let (cursor, console) = self.caption.as_mut().ok_or(RenderError::InvalidFrame)?;
                if cursor.step(&mut renderer.icons, target).map_err(RenderError::Icon)? {
                    let console = *console;
                    self.caption = None;
                    if console {
                        let light = frame.appearance.theme == Theme::Dark
                            || frame.appearance.hovered == Some(Control::Close)
                            || frame.appearance.pressed == Some(Control::Close);
                        self.caption = Some((
                            CaptionCursor::new(
                                frame.scale,
                                CaptionIcon::Close,
                                if light { [255; 3] } else { [0; 3] },
                            )
                            .map_err(RenderError::Icon)?,
                            false,
                        ));
                    } else {
                        self.stage = Stage::Done;
                    }
                }
            }
            Stage::Done => return Ok(true),
            Stage::Clear => return Err(RenderError::InvalidFrame),
        }
        Ok(false)
    }
    /// Distinguish text's own allocation claims from generic raster scratch admission.
    pub(super) fn status_domain(&self) -> bool {
        matches!(self.stage, Stage::StatusPrepare | Stage::Status)
    }
    /// Establish the next mask owner after the preceding cursor is released.
    fn begin_progress(&mut self) -> Result<(), RenderError> {
        let frame = &self.frame;
        self.progress = Some(ProgressCursor::new(
            frame.width,
            frame.height,
            frame.appearance,
            frame.feedback,
            frame.scale as f32,
        )?);
        self.stage = Stage::Progress;
        Ok(())
    }
    /// Transfer the completed target in constant time after all owner scratch is dropped.
    pub(super) fn finish(self) -> Result<Pixmap, RenderError> {
        if !matches!(self.stage, Stage::Done) {
            return Err(RenderError::InvalidFrame);
        }
        self.target.ok_or(RenderError::InvalidFrame)
    }
}
