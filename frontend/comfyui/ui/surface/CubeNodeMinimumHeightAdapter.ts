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
/** Adapt content-derived minimum height to Comfy's real Cube node geometry. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { CUBE_BASE_MINIMUM_HEIGHT } from './CubeSurfaceMinimumHeight.js';

/** Expand an undersized Cube without changing its width or graph origin. */
export function enforceCubeNodeMinimumHeight(node: CubeNode, minimumHeight: number): boolean {
  const currentHeight = finitePositive(Number(node.size[1]), CUBE_BASE_MINIMUM_HEIGHT);
  const requiredHeight = Math.ceil(
    Math.max(CUBE_BASE_MINIMUM_HEIGHT, finitePositive(minimumHeight, CUBE_BASE_MINIMUM_HEIGHT)),
  );
  if (currentHeight >= requiredHeight) return false;
  const size: [number, number] = [finitePositive(Number(node.size[0]), 1), requiredHeight];
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
