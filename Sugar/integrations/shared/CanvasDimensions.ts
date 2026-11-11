//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Calculate the same dimension proposal for workflow commands and their menu previews. */
import type {
  CanvasDimensionLimits,
  CanvasDimensions,
} from '../contracts/CanvasProtocol.generated.js';
import type { CanvasDimensionProposal } from '../contracts/CanvasGeometry.js';

/** Round to the declared positive field grid without escaping its bounds. */
export function constrainCanvasDimension(value: number, limits: CanvasDimensionLimits): number {
  if (
    ![limits.minimum, limits.maximum, limits.multiple].every(
      (limit) => Number.isSafeInteger(limit) && limit > 0,
    )
  )
    throw new Error('Dimension limits must be positive integer pixel values.');
  const minimum = Math.ceil(limits.minimum / limits.multiple) * limits.multiple;
  const maximum = Math.floor(limits.maximum / limits.multiple) * limits.multiple;
  if (!Number.isFinite(value) || value <= 0 || minimum > maximum)
    throw new Error('These dimensions cannot satisfy the field limits.');
  return Math.min(
    maximum,
    Math.max(minimum, Math.round(value / limits.multiple) * limits.multiple),
  );
}

/** Calculate actual destination dimensions before either owner applies a change. */
export function canvasSizingProposal(
  request: CanvasDimensionProposal,
  scale = request.scale,
): CanvasDimensions {
  if (scale.length > 32 || !/^[0-9]+(?:\.[0-9]+)?$/.test(scale))
    throw new Error('Enter a positive decimal scale factor of at most 32 characters.');
  const factor = Number(scale);
  if (!Number.isFinite(factor) || factor <= 0 || factor > 1000)
    throw new Error('Enter a scale factor greater than zero and no greater than 1000.');
  const source = request.direction === 'to-canvas' ? request.generation : request.reference;
  const multiplier = request.direction === 'to-canvas' ? factor : 1 / factor;
  return {
    width: constrainCanvasDimension(source.width * multiplier, request.widthLimits),
    height: constrainCanvasDimension(source.height * multiplier, request.heightLimits),
  };
}
