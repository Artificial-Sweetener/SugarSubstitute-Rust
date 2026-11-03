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
/** Fit an imported picture into a mask surface as grayscale coverage. */
/** Scale a source to fit inside the surface without cropping, centred, keeping its aspect ratio. */
export function aspectFit(sourceWidth, sourceHeight, width, height) {
    if (sourceWidth <= 0 || sourceHeight <= 0 || width <= 0 || height <= 0)
        throw new RangeError('Mask import sizes must be positive.');
    const scale = Math.min(width / sourceWidth, height / sourceHeight);
    const fitted = {
        width: Math.max(1, Math.round(sourceWidth * scale)),
        height: Math.max(1, Math.round(sourceHeight * scale)),
    };
    return {
        left: Math.floor((width - fitted.width) / 2),
        top: Math.floor((height - fitted.height) / 2),
        ...fitted,
    };
}
/**
 * Convert drawn RGBA pixels to opaque grayscale: each pixel's luminance, weighted by its alpha
 * over black, becomes its coverage, so white is fully masked and transparency is unmasked.
 */
export function grayscaleMask(rgba) {
    if (rgba.length % 4)
        throw new RangeError('Mask import pixels must be RGBA.');
    const output = new Uint8Array(rgba.length);
    for (let index = 0; index < rgba.length; index += 4) {
        const luminance = 0.299 * (rgba[index] ?? 0) + 0.587 * (rgba[index + 1] ?? 0) + 0.114 * (rgba[index + 2] ?? 0);
        const value = Math.round((luminance * (rgba[index + 3] ?? 0)) / 255);
        output[index] = output[index + 1] = output[index + 2] = value;
        output[index + 3] = 255;
    }
    return output;
}
