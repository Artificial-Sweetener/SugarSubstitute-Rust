// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The brush paint engine: dab placement along strokes and their compositing into coverage.
mod dabs;
mod stroke;
pub(crate) use dabs::{Contact, SegmentDabs, pressure_diameter, segment};
pub(crate) use stroke::{DabStyle, StrokeRaster};
#[cfg(test)]
mod tests;
