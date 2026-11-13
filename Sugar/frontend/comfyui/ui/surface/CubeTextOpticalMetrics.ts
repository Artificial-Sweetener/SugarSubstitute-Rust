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
/** Align title typography around cap height without letting descenders change the reference center. */

export interface CubeCapMetrics {
  actualBoundingBoxAscent: number;
  actualBoundingBoxDescent: number;
  fontBoundingBoxAscent: number;
  fontBoundingBoxDescent: number;
}

/** Place an alphabetic baseline so the measured capital letter occupies the requested center. */
export function cubeCapBaseline(center: number, metrics: CubeCapMetrics, fontSize: number): number {
  const ascent = metrics.actualBoundingBoxAscent;
  const descent = metrics.actualBoundingBoxDescent;
  return center + (Number.isFinite(ascent + descent) ? (ascent - descent) / 2 : fontSize * 0.35);
}

/** Correct a centered CSS line box using the font's ascent, descent, and capital-letter bounds. */
export function cubeCapLineOffset(metrics: CubeCapMetrics): number {
  const offset =
    (metrics.actualBoundingBoxAscent -
      metrics.actualBoundingBoxDescent -
      metrics.fontBoundingBoxAscent +
      metrics.fontBoundingBoxDescent) /
    2;
  return Number.isFinite(offset) ? offset : 0;
}
