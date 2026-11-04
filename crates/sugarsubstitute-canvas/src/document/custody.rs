// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge the coverage an edit carries to the document that keeps it.
use super::{DocumentEdit, Layer, LayerContent};
use crate::CanvasError;
use sugarsubstitute_contracts::execution::MemoryLease;

impl DocumentEdit {
    /// The same edit with every mask, layer mask and selection coverage it carries charged to
    /// `memory`, the lease of the document it will be published in.
    /// # Errors
    /// Rejects coverage `memory` cannot hold.
    pub(crate) fn adopt(self, memory: &MemoryLease) -> Result<Self, CanvasError> {
        Ok(match self {
            Self::Insert { index, layer } => Self::Insert { index, layer: layer.adopt(memory)? },
            Self::Replace(layer) => Self::Replace(layer.adopt(memory)?),
            Self::Select(selection) => {
                Self::Select(selection.map(|coverage| coverage.adopt(memory)).transpose()?)
            }
            Self::Group(edits) => Self::Group(
                edits.into_iter().map(|edit| edit.adopt(memory)).collect::<Result<_, _>>()?,
            ),
            kept @ (Self::Remove(_) | Self::Reorder { .. }) => kept,
        })
    }
}

impl Layer {
    /// The same layer with its mask coverage and layer mask charged to `memory`.
    fn adopt(self, memory: &MemoryLease) -> Result<Self, CanvasError> {
        let content = match self.content {
            LayerContent::Coverage { coverage, appearance } => {
                LayerContent::Coverage { coverage: coverage.adopt(memory)?, appearance }
            }
            other => other,
        };
        let mask = self.mask.map(|mask| mask.adopt(memory)).transpose()?;
        Ok(Self { content, mask, ..self })
    }
}
