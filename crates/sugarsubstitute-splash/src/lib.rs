// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Present the startup splash on CPU without owning startup or native windows.

mod animation;
pub mod assets;
mod caption_icons;
mod character;
mod chrome;
mod layout;
mod progress;
mod projection;
mod render;
mod shapes;
mod text;

pub use animation::{AnimationFrame, FlipPhase, POSE_COUNT, PaperFlip};
pub use caption_icons::IconError;
pub use character::{CharacterError, CharacterLibrary, PoseAsset};
pub use layout::{Appearance, Control, LOGICAL_SIZE, Theme};
pub use progress::ProgressFrame;
pub use projection::{Bounds, Point, projected_quad};
pub use render::{RenderError, SplashRenderer};
pub use sugarsubstitute_contracts::localization::{LOCALES, Messages, messages};
