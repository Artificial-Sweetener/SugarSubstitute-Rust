import { decodeCanvasRaster, encodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
/** Retain straight image colors under selected coverage and remove unselected content from RGB-only consumers. */
export function canvasSelectionImage(image, selection) {
    if (image.format !== 'rgba8' ||
        selection.format !== 'coverage8' ||
        image.width !== selection.width ||
        image.height !== selection.height)
        throw new Error('Selection image content and coverage must share one full-canvas frame.');
    const rgba = decodeCanvasRaster(image), coverage = decodeCanvasRaster(selection), result = new Uint8Array(rgba.length);
    for (let index = 0; index < coverage.length; index++) {
        const offset = index * 4, alpha = Math.round(((rgba[offset + 3] ?? 0) * (coverage[index] ?? 0)) / 255);
        // Comfy IMAGE outputs discard alpha; keep content outside the selected shape out of those outputs too.
        if (alpha) {
            result[offset] = rgba[offset] ?? 0;
            result[offset + 1] = rgba[offset + 1] ?? 0;
            result[offset + 2] = rgba[offset + 2] ?? 0;
        }
        result[offset + 3] = alpha;
    }
    return encodeCanvasRaster(image.width, image.height, 'rgba8', result);
}
