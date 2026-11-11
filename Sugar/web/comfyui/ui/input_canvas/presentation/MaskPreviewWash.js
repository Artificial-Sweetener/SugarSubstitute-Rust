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
/** Composite effective mask coverage once over the associated loader's full-color image. */
import { decodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';
/** Apply one half-opacity neutral wash, retaining soft coverage and straight-alpha image pixels. */
export function maskPreviewPixels(mask, image, placement = null) {
    if (mask.format !== 'coverage8' || image.format !== 'rgba8')
        throw new Error('A mask preview requires an effective monochrome mask and a color image.');
    if (image.width > INPUT_PREVIEW_MAXIMUM || image.height > INPUT_PREVIEW_MAXIMUM)
        throw new Error('The image preview exceeds its display limit.');
    const coverage = decodeCanvasRaster(mask), source = decodeCanvasRaster(image), result = new Uint8ClampedArray(source.length);
    for (let y = 0; y < image.height; y++)
        for (let x = 0; x < image.width; x++) {
            const position = placement
                ? {
                    x: ((x + 0.5) * placement.frame.width) / image.width - placement.bounds.left,
                    y: ((y + 0.5) * placement.frame.height) / image.height - placement.bounds.top,
                }
                : null;
            const outside = position &&
                placement &&
                (position.x < 0 ||
                    position.y < 0 ||
                    position.x >= placement.bounds.width ||
                    position.y >= placement.bounds.height);
            const value = outside
                ? 0
                : sampleCoverage(coverage, mask.width, mask.height, position && placement
                    ? (position.x * mask.width) / placement.bounds.width - 0.5
                    : ((x + 0.5) * mask.width) / image.width - 0.5, position && placement
                    ? (position.y * mask.height) / placement.bounds.height - 0.5
                    : ((y + 0.5) * mask.height) / image.height - 0.5);
            const offset = (y * image.width + x) * 4, retainedAlpha = ((source[offset + 3] ?? 0) / 255) * 0.5, alpha = 0.5 + retainedAlpha;
            for (let channel = 0; channel < 3; channel++)
                result[offset + channel] = Math.round((value * 0.5 + (source[offset + channel] ?? 0) * retainedAlpha) / alpha);
            result[offset + 3] = Math.round(alpha * 255);
        }
    return result;
}
/** Match pixel-center bilinear scaling when native loader output geometry differs from the image. */
function sampleCoverage(bytes, width, height, x, y) {
    x = Math.max(0, Math.min(width - 1, x));
    y = Math.max(0, Math.min(height - 1, y));
    const left = Math.floor(x), top = Math.floor(y), right = Math.min(left + 1, width - 1), bottom = Math.min(top + 1, height - 1), horizontal = x - left, vertical = y - top;
    const upper = (bytes[top * width + left] ?? 0) * (1 - horizontal) +
        (bytes[top * width + right] ?? 0) * horizontal, lower = (bytes[bottom * width + left] ?? 0) * (1 - horizontal) +
        (bytes[bottom * width + right] ?? 0) * horizontal;
    return upper * (1 - vertical) + lower * vertical;
}
