// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse canonical input allocations for value-identical coverage and composition operations.
use super::*;

impl<S: RasterSource> Evaluator<S> {
    pub(super) fn alias_input(
        &mut self,
        operation: Operation,
        region: Region,
        context: &JobContext,
    ) -> Result<Option<Arc<Tile>>, ImageError> {
        let input = match operation {
            Operation::GaussianBlur { input, blur } if blur.sigma() == 0.0 => Some(input),
            Operation::Mask { input, mask } => match self.uniform_region(mask, region)? {
                Some(color) if color.channels()[3] == 1.0 => Some(input),
                Some(color) if color.channels()[3] == 0.0 => Some(mask),
                _ => None,
            },
            Operation::Mix { original, opacity: 0.0, .. } => Some(original),
            Operation::Over { background, foreground } => {
                let background_color = self.uniform_region(background, region)?;
                if background_color == Some(Pixel::TRANSPARENT) {
                    Some(foreground)
                } else if self.uniform_region(foreground, region)? == Some(Pixel::TRANSPARENT) {
                    Some(background)
                } else if background_color.is_some() {
                    let input = self.point_input(foreground, region, context)?;
                    if input.pixels().iter().all(|pixel| pixel.channels()[3] == 1.0) {
                        return Ok(Some(input));
                    }
                    None
                } else {
                    None
                }
            }
            _ => None,
        };
        input.map(|node| self.point_input(node, region, context)).transpose()
    }

    /// Follow only transparent prefixes; shared arbitrary graph branches are not recursively folded.
    fn uniform_region(&self, node: NodeId, region: Region) -> Result<Option<Pixel>, ImageError> {
        match self.graph.operation(node)? {
            Operation::Mask { mask, .. } => {
                Ok(Self::primitive_uniform(self.graph.operation(mask)?, region)
                    .filter(|color| color.channels()[3] == 0.0)
                    .map(|_| Pixel::TRANSPARENT))
            }
            Operation::Over { background, foreground }
                if Self::primitive_uniform(self.graph.operation(background)?, region)
                    == Some(Pixel::TRANSPARENT) =>
            {
                self.uniform_region(foreground, region)
            }
            operation => Ok(Self::primitive_uniform(operation, region)),
        }
    }

    fn primitive_uniform(operation: Operation, region: Region) -> Option<Pixel> {
        match operation {
            Operation::Solid { color, .. } => Some(color),
            Operation::Rectangle { shape, translation, .. } => {
                let bounds = shape.bounds();
                let left = (i64::from(region.x()) - i64::from(translation[0])) as f64;
                let top = (i64::from(region.y()) - i64::from(translation[1])) as f64;
                let right = (i64::from(region.right()) - i64::from(translation[0])) as f64;
                let bottom = (i64::from(region.bottom()) - i64::from(translation[1])) as f64;
                if shape.color().channels()[3] == 0.0
                    || bounds[2] <= left
                    || bounds[3] <= top
                    || bounds[0] >= right
                    || bounds[1] >= bottom
                {
                    Some(Pixel::TRANSPARENT)
                } else if bounds[0] <= left
                    && bounds[1] <= top
                    && bounds[2] >= right
                    && bounds[3] >= bottom
                {
                    Some(shape.color())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
