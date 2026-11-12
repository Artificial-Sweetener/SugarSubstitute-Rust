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
/** Prepare a bounded decorative contrast variant through the shared icon tone policy. */
import { classifyCubeIconTone, invertNeutralCubeIconPixels, } from './CubeIconToneProfile.js';
const SAMPLE_SIZE = 32;
const RENDER_SIZE = 192;
const contrastImages = new WeakMap();
/** Profile each decoded image once, retaining original artwork when canvas access is unavailable. */
export function resolveCubeIconContrastImage(image) {
    const retained = contrastImages.get(image);
    if (retained)
        return retained;
    if (image.naturalWidth <= 0 || image.naturalHeight <= 0)
        return null;
    try {
        const sample = drawIcon(image.ownerDocument, image, SAMPLE_SIZE);
        if (!sample)
            return null;
        const tone = classifyCubeIconTone(sample.context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data);
        const variant = tone === 'original' ? null : drawIcon(image.ownerDocument, image, RENDER_SIZE);
        if (tone !== 'original' && !variant)
            return null;
        if (variant) {
            const pixels = variant.context.getImageData(0, 0, RENDER_SIZE, RENDER_SIZE);
            pixels.data.set(invertNeutralCubeIconPixels(pixels.data));
            variant.context.putImageData(pixels, 0, 0);
        }
        const result = { tone, canvas: variant?.canvas ?? null };
        contrastImages.set(image, result);
        return result;
    }
    catch (error) {
        console.warn('SugarCubes retained an icon whose contrast could not be sampled.', { error });
        const original = { tone: 'original', canvas: null };
        contrastImages.set(image, original);
        return original;
    }
}
/** Adapt one already resolved image without changing its source, fetching again or observing the document. */
export function prepareCubeIconContrast(root) {
    const image = root.querySelector('img');
    if (!image)
        return;
    const prepare = () => {
        if (!root.contains(image) ||
            root.dataset.iconTone ||
            image.naturalWidth <= 0 ||
            image.naturalHeight <= 0)
            return;
        const variant = resolveCubeIconContrastImage(image);
        if (!variant)
            return;
        if (variant.canvas) {
            variant.canvas.className = 'sugarcubes-cube-icon__contrast';
            variant.canvas.setAttribute('aria-hidden', 'true');
            root.append(variant.canvas);
        }
        root.dataset.iconTone = variant.tone;
    };
    if (image.complete)
        prepare();
    else
        image.addEventListener('load', prepare, { once: true });
}
/** Preserve aspect ratio inside a fixed bounded canvas for both profiling and display variants. */
function drawIcon(document, image, size) {
    const canvas = document.createElement('canvas');
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context)
        return null;
    const scale = size / Math.max(image.naturalWidth, image.naturalHeight);
    const width = image.naturalWidth * scale;
    const height = image.naturalHeight * scale;
    context.drawImage(image, (size - width) / 2, (size - height) / 2, width, height);
    return { canvas, context };
}
