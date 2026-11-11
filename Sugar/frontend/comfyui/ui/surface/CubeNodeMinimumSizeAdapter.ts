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
/** Adapt presentation-derived minimum size to Comfy's real Cube node geometry. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { CUBE_BASE_MINIMUM_HEIGHT, CUBE_MINIMUM_WIDTH } from './CubeSurfaceMinimumHeight.js';

/** Expand an undersized Cube without changing its graph origin. */
export function enforceCubeNodeMinimumSize(
  node: CubeNode,
  minimumSize: readonly [number, number],
): boolean {
  const currentWidth = finitePositive(Number(node.size[0]), CUBE_MINIMUM_WIDTH);
  const currentHeight = finitePositive(Number(node.size[1]), CUBE_BASE_MINIMUM_HEIGHT);
  const requiredWidth = Math.ceil(
    Math.max(CUBE_MINIMUM_WIDTH, finitePositive(minimumSize[0], CUBE_MINIMUM_WIDTH)),
  );
  const requiredHeight = Math.ceil(
    Math.max(CUBE_BASE_MINIMUM_HEIGHT, finitePositive(minimumSize[1], CUBE_BASE_MINIMUM_HEIGHT)),
  );
  if (currentWidth >= requiredWidth && currentHeight >= requiredHeight) return false;
  const size: [number, number] = [
    Math.max(currentWidth, requiredWidth),
    Math.max(currentHeight, requiredHeight),
  ];
  node.setSize?.([...size]);
  node.size[0] = size[0];
  node.size[1] = size[1];
  node.onResize?.([...size]);
  return true;
}

/** Return one positive host geometry value or its finite fallback. */
function finitePositive(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}
