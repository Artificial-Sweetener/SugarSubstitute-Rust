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
/** Derive Cube reading order exclusively from accepted native graph positions. */
import type { CubeGraphAnalysis, CubeGraphPlacement } from './CubeGraphModels.js';

/** Keep every Cube eligible regardless of ports, execution dependencies or segment membership. */
export function cubeGraphOrder(analysis: CubeGraphAnalysis): readonly string[] {
  return cubePlacementOrder(analysis.placements);
}

/** Apply canonical position ordering to native placements before or after backend analysis. */
export function cubePlacementOrder(placements: readonly CubeGraphPlacement[]): readonly string[] {
  return [...placements]
    .sort(
      (left, right) =>
        left.position[0] - right.position[0] ||
        left.position[1] - right.position[1] ||
        (left.instanceId < right.instanceId ? -1 : left.instanceId > right.instanceId ? 1 : 0),
    )
    .map((placement) => placement.instanceId);
}
