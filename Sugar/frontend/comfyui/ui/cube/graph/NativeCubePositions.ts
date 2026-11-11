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
/** Own validated native position writes and compensation while retaining coordinate vectors. */
import type { ComfyNode } from '../../types/graph.js';

/** Require both finite coordinates while accepting native typed vectors. */
export function readNativeCubePosition(value: unknown): readonly [number, number] {
  if (!Array.isArray(value) && !(value instanceof Float32Array) && !(value instanceof Float64Array))
    throw new TypeError('Cube reorder position must be a native coordinate vector.');
  const [x, y] = value;
  if (typeof x !== 'number' || typeof y !== 'number' || !Number.isFinite(x) || !Number.isFinite(y))
    throw new TypeError('Cube reorder position must contain finite coordinates.');
  return [x, y];
}

/** Use the native setter for geometry notification and retain the existing coordinate vector. */
export function writeNativeCubePosition(
  node: ComfyNode,
  position: readonly [number, number],
): void {
  node.setPos?.(position[0], position[1]);
  restoreNativeCubePosition(node, position);
}

/** Restore coordinates without invoking a setter that has already failed. */
export function restoreNativeCubePosition(
  node: ComfyNode,
  position: readonly [number, number],
): void {
  if (!node.pos) throw new TypeError('Cube reorder target has no native position.');
  node.pos[0] = position[0];
  node.pos[1] = position[1];
}
