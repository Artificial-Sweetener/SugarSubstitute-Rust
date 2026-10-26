// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own admitted GPU resources for the shared image DAG on one explicitly selected device.
//!
//! Application services submit discovery, initialization and evaluation through execution.
//! This adapter creates no runtime or worker. Native windows remain with the desktop adapter.
mod adapter;
mod batch;
mod buffer;
mod coordination;
mod dependency;
mod display;
mod display_image;
mod display_mapping;
mod display_readback;
mod error;
mod evaluate;
mod fault;
mod kernel;
mod native_commands;
mod request;
mod resample;
mod selection;
mod surface_state;
mod timing;
mod transfer;
mod window;
mod window_configuration;
mod window_frame;
mod window_shader;

#[cfg(feature = "boundary-fixtures")]
pub mod boundary_fixtures;

pub use adapter::{AdapterCatalog, AdapterClass, AdapterDescriptor, AdapterId, Backend, GpuDevice};
pub use batch::GpuWork;
pub use buffer::{GpuTile, Readback};
pub use display::{DisplayRegions, DisplayTile};
pub use display_image::{DisplayCopy, DisplayImage};
pub use display_mapping::DisplayMapping;
pub use display_readback::{DisplayReadback, DisplayReadbackWorkspace};
pub use error::GpuError;
pub use evaluate::GpuEvaluator;
pub use request::GpuRegions;
pub use selection::AdapterKey;
pub use timing::GpuDiagnostics;
pub use window::{NativeWindowHandle, WindowFrame, WindowPresentation, WindowSurface};
