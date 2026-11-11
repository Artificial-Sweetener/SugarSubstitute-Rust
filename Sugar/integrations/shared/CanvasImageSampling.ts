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
/** Sample coverage and premultiplied color in one declared processing frame. */
import type { CanvasImageRectangle } from '../contracts/CanvasImageRegion.js';

/** Bilinearly sample one component, clamping only at the declared unpadded image edges. */
export function sampleCanvasComponent(
  bytes: Uint8Array,
  width: number,
  components: number,
  component: number,
  x: number,
  y: number,
  bounds: CanvasImageRectangle,
  premultiply = false,
): number {
  const left = Math.max(bounds.left, Math.min(bounds.left + bounds.width - 1, x));
  const top = Math.max(bounds.top, Math.min(bounds.top + bounds.height - 1, y));
  const x0 = Math.floor(left),
    y0 = Math.floor(top);
  const x1 = Math.min(x0 + 1, bounds.left + bounds.width - 1);
  const y1 = Math.min(y0 + 1, bounds.top + bounds.height - 1);
  const dx = left - x0,
    dy = top - y0;
  const value = (column: number, row: number): number => {
    const offset = (row * width + column) * components;
    return (bytes[offset + component] ?? 0) * (premultiply ? (bytes[offset + 3] ?? 0) / 255 : 1);
  };
  return (
    value(x0, y0) * (1 - dx) * (1 - dy) +
    value(x1, y0) * dx * (1 - dy) +
    value(x0, y1) * (1 - dx) * dy +
    value(x1, y1) * dx * dy
  );
}
