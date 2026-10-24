// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose installer pages from the shared native widgets and application-owned navigation.
//! Geometry is logical; the desktop owns native peers and the execution adapter owns preparation.
mod builder;
mod controls;
pub mod driver;
mod pages;
mod preparation;
mod render;
mod shell;
mod snapshot;
use builder::Builder;
pub use render::{Painting, Prepared};
pub use snapshot::{SNAPSHOT_BYTES, Snapshot};
use std::{io, time::Duration};
use sugarsubstitute_application::setup_flow::{
    OptionField, Page, PathField, RuntimeMode, SetupFlow,
};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    localization::installer::{self, InstallerText as Text},
};
use sugarsubstitute_ui::{
    form::{Control, ControlId},
    scene::{Element, SceneResources},
    text::PaintSpec,
    widgets::Bounds,
};

/// Installer intent associated with one stable native peer; this grants no storage authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Close this installer and settle its owned work.
    Close,
    /// Advance the current installation page.
    Primary,
    /// Return to the preceding page.
    Back,
    /// Select a supported display language.
    Locale,
    /// Edit one installation/runtime path.
    Path(PathField),
    /// Open the shell picker for one editable path.
    Browse(PathField),
    /// Select managed or attached ComfyUI ownership.
    Runtime(RuntimeMode),
    /// Choose an upstream provisioning option.
    Option(OptionField),
    /// Enable regional download mirrors.
    Mirrors,
    /// Reveal or collapse the native output control.
    Details,
}
impl Action {
    /// Stable native text observation slot shared by field composition and installer input.
    pub const fn text_slot(self) -> Option<usize> {
        match self {
            Self::Path(PathField::Installation) => Some(0),
            Self::Path(PathField::Comfy) => Some(1),
            Self::Path(PathField::Python) => Some(2),
            Self::Path(PathField::Models) => Some(3),
            Self::Path(PathField::Outputs) => Some(4),
            _ => None,
        }
    }
    /// Stable peer identity across page changes; no display strings serve as command identifiers.
    pub fn id(self) -> ControlId {
        let field = |field| match field {
            PathField::Installation => 0,
            PathField::Comfy => 1,
            PathField::Python => 2,
            PathField::Models => 3,
            PathField::Outputs => 4,
        };
        let id = match self {
            Self::Close => 1,
            Self::Primary => 2,
            Self::Back => 3,
            Self::Locale => 22,
            Self::Path(value) => 5 + field(value),
            Self::Browse(value) => 10 + field(value),
            Self::Runtime(RuntimeMode::Managed) => 15,
            Self::Runtime(RuntimeMode::Attached) => 16,
            Self::Option(OptionField::Release) => 17,
            Self::Option(OptionField::ComfyVersion) => 18,
            Self::Option(OptionField::Variant) => 19,
            Self::Mirrors => 20,
            Self::Details => 21,
        };
        // Every branch is a distinct compile-time nonzero peer identifier.
        ControlId::from_nonzero(std::num::NonZeroU16::MIN.saturating_add(id - 1))
    }
}

/// Pointer/focus observations supplied by the native peer owner for one immutable paint request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interaction {
    /// Enabled peer under the pointer.
    pub hovered: Option<ControlId>,
    /// Enabled peer retaining the current press gesture.
    pub pressed: Option<ControlId>,
    /// Actual native focused peer.
    pub focused: Option<ControlId>,
}

/// Validated client viewport and application-relative elapsed time for preparation.
#[derive(Clone, Copy)]
pub struct View {
    /// Physical client dimensions and logical-to-device scale.
    pub surface: PaintSpec,
    /// Requested logical page scroll position, clamped against measured content.
    pub scroll: f32,
    /// Monotonic application time, used only for long-wait feedback.
    pub now: Duration,
    /// Actual native observations; presentation cannot create focus or selection authority.
    pub interaction: Interaction,
    /// Original native document revisions for installation, ComfyUI, Python, models and outputs.
    pub text_revisions: [u64; 5],
}

/// One complete immutable page composition, whose commands and native geometry share admission.
pub struct Scene {
    /// Whole-window material, branding, journey and footer.
    pub shell: Vec<Element>,
    /// Page elements relative to the viewport origin, including its clamped scroll displacement.
    pub page: Vec<Element>,
    /// Retained peer geometry and scroll facts for the same application revision.
    pub layout: Layout,
    surface: PaintSpec,
}

/// Native peer geometry retained after scene commands have become validated pixels.
pub struct Layout {
    /// Prepared branding area admitting native window movement.
    pub drag_bounds: Bounds,
    /// Page viewport in logical whole-window coordinates.
    pub viewport: Bounds,
    /// Logical page width excluding the scrollbar gutter when content overflows vertically.
    pub page_width: f32,
    /// Whole-window peers, whose identity survives page/layout changes.
    pub shell_controls: Vec<Control>,
    /// Page peers relative to the viewport; native adapters clip them with their parent.
    pub page_controls: Vec<Control>,
    /// Measured page height before scrolling.
    pub content_height: f32,
    /// Applied logical page scroll offset.
    pub scroll: f32,
    /// Native console rectangle relative to the page viewport, when revealed.
    pub console: Option<Bounds>,
    _metadata: Option<MemoryClaim>,
}

/// Admitted upper bound for one page's commands, peer values and localized layout metadata.
/// Raster masks, glyph output and native copies require their own shared-renderer reservations.
pub const METADATA_BYTES: u64 = 512 * 1024;

/// Prepare the current page through the same shared widget and font owners as the splash.
/// Production callers supply the admitted preparation lease; offline raster qualification may omit it.
/// # Errors
/// Rejects invalid viewport/scroll input and preserves measurement/admission failures.
pub fn compose(
    state: &Snapshot,
    resources: &mut SceneResources,
    lease: Option<&MemoryLease>,
) -> io::Result<Scene> {
    let view = state.view;
    let s = view.surface;
    let width = s.width as f32 / s.scale;
    let height = s.height as f32 / s.scale;
    if !s.scale.is_finite()
        || !(0.5..=4.0).contains(&s.scale)
        || s.width > 4096
        || s.height > 4096
        || width < 720.0
        || height < 560.0
        || !view.scroll.is_finite()
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid installer viewport"));
    }
    let metadata =
        lease.map(|lease| lease.claim(METADATA_BYTES)).transpose().map_err(io::Error::other)?;
    let mut builder = Builder {
        state,
        view,
        resources,
        lease,
        copy: installer::messages(state.locale),
        elements: Vec::new(),
        controls: Vec::new(),
        console: None,
    };
    let viewport = builder.shell(width, height)?;
    let shell = std::mem::take(&mut builder.elements);
    let shell_controls = std::mem::take(&mut builder.controls);
    let mut page_width = viewport.width;
    let mut content_height = builder.page(page_width, viewport.height)?;
    if content_height > viewport.height {
        page_width -= sugarsubstitute_ui::form::SCROLL_GUTTER;
        builder.elements.clear();
        builder.controls.clear();
        builder.console = None;
        content_height = builder.page(page_width, viewport.height)?;
    }
    let scroll = view.scroll.clamp(0.0, (content_height - viewport.height).max(0.0));
    let centered = if state.page == Page::Progress && state.details_open {
        0.0
    } else {
        ((viewport.height - content_height) * 0.5).max(0.0)
    };
    builder.translate(centered - scroll);
    sugarsubstitute_ui::form::validate(&shell_controls, s.scale)?;
    sugarsubstitute_ui::form::validate(&builder.controls, s.scale)?;
    Ok(Scene {
        shell,
        page: builder.elements,
        surface: s,
        layout: Layout {
            drag_bounds: bounds_for_caption(width),
            viewport,
            page_width,
            shell_controls,
            page_controls: builder.controls,
            content_height,
            scroll,
            console: builder.console,
            _metadata: metadata,
        },
    })
}

/// Canonical localized installer labels; upstream and user values are retained verbatim.
type Copy = &'static installer::InstallerMessages;

#[cfg(test)]
mod tests;

/// Keep caption input and body placement tied to the same authored branding geometry.
fn bounds_for_caption(width: f32) -> Bounds {
    Bounds { x: 0.0, y: 0.0, width, height: shell::BRAND_HEIGHT }
}
