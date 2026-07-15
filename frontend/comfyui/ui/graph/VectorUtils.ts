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
/**
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/VectorUtils.js`.
 */

import type { Vec2 } from '../types/common.js';

/**
 * Read vector2.
 */
export function readVector2(vec: unknown, fallbackX = 0, fallbackY = 0): Vec2 {
  if (!Array.isArray(vec) || vec.length < 2) {
    return [fallbackX, fallbackY];
  }
  const x = Number(vec[0]);
  const y = Number(vec[1]);
  return [Number.isFinite(x) ? x : fallbackX, Number.isFinite(y) ? y : fallbackY];
}

/**
 * Coerce vec2.
 */
export function coerceVec2(value: unknown): Vec2 | null {
  if (!Array.isArray(value) || value.length < 2) {
    return null;
  }
  const x = Number(value[0]);
  const y = Number(value[1]);
  if (!Number.isFinite(x) || !Number.isFinite(y)) {
    return null;
  }
  return [x, y];
}
