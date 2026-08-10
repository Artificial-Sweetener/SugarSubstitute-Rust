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
/** Compute collision-free placements for adjacent Cube-node swaps. */

import type { Vec2 } from '../../types/common.js';

export interface CubeNodeSwapBounds {
  position: Vec2;
  width: number;
}

export interface CubeNodeSwapPlacements {
  formerLeft: Vec2;
  formerRight: Vec2;
}

/** Exchange adjacent horizontal slots while preserving a safe inter-node gap. */
export function resolveCubeNodeSwapPlacements(
  formerLeft: CubeNodeSwapBounds,
  formerRight: CubeNodeSwapBounds,
  minimumGap: number,
): CubeNodeSwapPlacements {
  const authoredGap = formerRight.position[0] - (formerLeft.position[0] + formerLeft.width);
  const gap = Math.max(nonNegative(minimumGap), nonNegative(authoredGap));
  return {
    formerLeft: [formerLeft.position[0] + formerRight.width + gap, formerRight.position[1]],
    formerRight: [formerLeft.position[0], formerLeft.position[1]],
  };
}

/** Keep invalid host geometry from creating negative spacing. */
function nonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}
