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
/** Define pixel-grid centering shared by native canvas adapters. */
/** Center an intrinsic image rectangle, retaining negative offsets and transparent margins. */
export function centerCanvasImage(
  canvas: { readonly width: number; readonly height: number },
  image: { readonly width: number; readonly height: number },
): { readonly left: number; readonly top: number } {
  for (const value of [canvas.width, canvas.height, image.width, image.height])
    if (!Number.isSafeInteger(value) || value < 1 || value > 300000)
      throw new RangeError('Canvas and image dimensions must be positive integer pixels.');
  return Object.freeze({
    left: Math.floor((canvas.width - image.width) / 2),
    top: Math.floor((canvas.height - image.height) / 2),
  });
}

/** Preserve intrinsic pixels at an explicit captured origin, using centering only for ordinary generation. */
export function placeCanvasImage(
  canvas: { readonly width: number; readonly height: number },
  image: { readonly width: number; readonly height: number },
  position: { readonly left: number; readonly top: number } | null,
): { readonly left: number; readonly top: number } {
  if (!position) return centerCanvasImage(canvas, image);
  if (
    ![position.left, position.top].every(
      (value) => Number.isSafeInteger(value) && Math.abs(value) <= 300000,
    )
  )
    throw new Error('Invalid canvas image position.');
  return Object.freeze({ ...position });
}
