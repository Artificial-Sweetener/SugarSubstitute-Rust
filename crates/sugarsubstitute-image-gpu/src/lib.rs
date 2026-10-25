// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own admitted GPU resources for the shared image DAG on one explicitly selected device.
//!
//! Application services submit discovery, initialization and evaluation through execution.
//! This adapter creates no runtime or worker. Native windows remain with the desktop adapter.
mod adapter;
mod batch;
mod buffer;
mod dependency;
mod display;
mod error;
mod evaluate;
mod fault;
mod kernel;
mod request;
mod resample;
mod selection;
mod transfer;

pub use adapter::{AdapterCatalog, AdapterClass, AdapterDescriptor, AdapterId, Backend, GpuDevice};
pub use batch::GpuWork;
pub use buffer::{GpuTile, Readback};
pub use display::{DisplayReadback, DisplayRegions, DisplayTile};
pub use error::GpuError;
pub use evaluate::GpuEvaluator;
pub use request::GpuRegions;
pub use selection::AdapterKey;
