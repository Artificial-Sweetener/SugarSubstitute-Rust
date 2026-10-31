// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fix native presentation ownership before a canvas session admits display work.

use super::*;
use sugarsubstitute_desktop::canvas::WindowTarget;

/// Only GPU presentation retains a window target; an embedded composition surface is owned by
/// the desktop adapter and acknowledges the frames it accepts.
pub(super) enum Presentation {
    /// No native consumer will acknowledge frames.
    Headless,
    /// A desktop-owned surface will acknowledge CPU frames.
    CpuSurface,
    /// The main-window composition will acknowledge CPU frames and shared GPU textures.
    #[cfg(all(windows, feature = "native-composition"))]
    Composition,
    /// A GPU device must attach to the retained standalone window.
    GpuWindow(WindowTarget),
}

impl Presentation {
    /// Whether publication requires acceptance by a native presenter.
    pub(super) fn attached(&self) -> bool {
        !matches!(self, Self::Headless)
    }

    /// Borrow the window only when the selected GPU path needs it.
    pub(super) fn gpu_window(&self) -> Option<&WindowTarget> {
        match self {
            Self::GpuWindow(target) => Some(target),
            Self::Headless | Self::CpuSurface => None,
            #[cfg(all(windows, feature = "native-composition"))]
            Self::Composition => None,
        }
    }

    /// Whether frames are presented by the main-window composition.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(super) const fn composition(&self) -> bool {
        matches!(self, Self::Composition)
    }
}

impl Session {
    /// Attach a desktop-owned CPU surface before any display work is admitted.
    /// The presenter must acknowledge each accepted frame through `did_present_cpu`.
    ///
    /// # Errors
    /// Rejects GPU sessions, duplicate or late attachments, and closing sessions.
    pub(crate) fn attach_cpu_surface(&mut self) -> Result<(), Box<dyn Error>> {
        if self.config.gpu.is_some() {
            return Err("Canvas CPU surface cannot attach to a GPU session".into());
        }
        self.attach_presentation(Presentation::CpuSurface)
    }

    /// Attach the main-window composition before any display work is admitted.
    /// It acknowledges CPU frames through `did_present_cpu` and shared GPU frames through
    /// `did_present_composition`; previews always remain CPU frames.
    ///
    /// # Errors
    /// Rejects standalone GPU requests, duplicate or late attachments, and closing sessions.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn attach_composition_surface(&mut self) -> Result<(), Box<dyn Error>> {
        if matches!(
            self.config.gpu,
            Some(crate::native_canvas::gpu_policy::GpuRequest::Required(_))
        ) {
            return Err("Canvas composition cannot attach to a standalone GPU session".into());
        }
        self.attach_presentation(Presentation::Composition)?;
        self.gpu_admitted = self.config.gpu.is_none();
        Ok(())
    }

    /// Admit the opted-in GPU once a composition-hosted main lifetime uses the canvas.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn admit_composition_gpu(&mut self) {
        self.gpu_admitted = true;
    }

    /// Fix presentation ownership before any frame or GPU device can be published.
    pub(super) fn attach_presentation(
        &mut self,
        presentation: Presentation,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing || self.presentation.attached() {
            return Err("Canvas native peer cannot be attached".into());
        }
        if self.device_job.is_some()
            || (self.config.gpu.is_some() && self.gpu_ready)
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.document.cpu_projection.is_some()
            || self.document.display_evidence.is_some()
            || self.document.native_presented.is_some()
        {
            return Err("Canvas native peer must attach before presentation starts".into());
        }
        self.presentation = presentation;
        Ok(())
    }
}
