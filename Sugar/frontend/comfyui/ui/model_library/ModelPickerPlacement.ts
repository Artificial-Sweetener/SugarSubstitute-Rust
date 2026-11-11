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
/** Match the reference's attached popup geometry within the available host boundary. */
export interface ModelPickerRectangle {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface ModelPickerPlacement extends ModelPickerRectangle {
  readonly mode: 'above' | 'below';
}

const PREFERRED_WIDTH = 560;
const PREFERRED_HEIGHT = 630;
const MARGIN = 8;

/** Drop the margin when it would consume the whole boundary, as the Qt reference does. */
function safeAxis(origin: number, length: number): readonly [number, number] {
  const bounded = Math.max(0, length);
  return bounded > MARGIN * 2
    ? [origin + MARGIN, origin + bounded - MARGIN]
    : [origin, origin + bounded];
}

/** Prefer a full attached side, then fit the larger side without imposing overflow-causing minima. */
export function modelPickerPlacement(
  boundary: ModelPickerRectangle,
  anchor: ModelPickerRectangle,
): ModelPickerPlacement {
  const [safeLeft, safeRight] = safeAxis(boundary.left, boundary.width);
  const [safeTop, safeBottom] = safeAxis(boundary.top, boundary.height);
  const width = Math.min(PREFERRED_WIDTH, Math.max(1, safeRight - safeLeft));
  const left = Math.max(safeLeft, Math.min(anchor.left, safeRight - width));
  const bottom = anchor.top + anchor.height;
  const below = Math.max(0, safeBottom - bottom);
  const above = Math.max(0, anchor.top - safeTop);
  const mode =
    below >= PREFERRED_HEIGHT
      ? 'below'
      : above >= PREFERRED_HEIGHT || above > below
        ? 'above'
        : 'below';
  const height = Math.min(PREFERRED_HEIGHT, Math.max(1, mode === 'below' ? below : above));
  return { left, top: mode === 'below' ? bottom : anchor.top - height, width, height, mode };
}
