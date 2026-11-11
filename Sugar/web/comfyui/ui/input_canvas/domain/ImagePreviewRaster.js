import { decodeCanvasRaster, encodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { INPUT_PREVIEW_MAXIMUM } from './InputPreviewResolution.js';
const linearSrgb = Float64Array.from(Array.from({ length: 256 }, (_, value) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}));
/** Encode averaged linear-light color after removing premultiplied alpha. */
function encodeColor(value) {
    const channel = value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055;
    return Math.max(0, Math.min(255, Math.round(channel * 255)));
}
/** Downsample validated image pixels without staging files or altering native mask-edit resolution.
 *
 * Args:
 *   source: Straight sRGB pixels from the same frozen capture as the mask.
 *   maximumSize: Longest display edge, or zero to retain the original raster.
 *
 * Returns:
 *   A bounded, area-averaged image with alpha-weighted color and the original aspect ratio.
 */
export function imagePreviewRaster(source, maximumSize) {
    if (source.format !== 'rgba8')
        throw new Error('An image preview requires image pixels.');
    if (!Number.isSafeInteger(maximumSize) || maximumSize < 0 || maximumSize > INPUT_PREVIEW_MAXIMUM)
        throw new Error('Invalid image preview size.');
    if (!maximumSize || Math.max(source.width, source.height) <= maximumSize)
        return source;
    const scale = maximumSize / Math.max(source.width, source.height);
    const width = Math.max(1, Math.round(source.width * scale));
    const height = Math.max(1, Math.round(source.height * scale));
    const pixels = decodeCanvasRaster(source), output = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y++) {
        const top = (y * source.height) / height, bottom = ((y + 1) * source.height) / height;
        for (let x = 0; x < width; x++) {
            const left = (x * source.width) / width, right = ((x + 1) * source.width) / width;
            let alpha = 0, red = 0, green = 0, blue = 0;
            for (let row = Math.floor(top); row < Math.min(source.height, Math.ceil(bottom)); row++) {
                const rowWeight = Math.min(row + 1, bottom) - Math.max(row, top);
                for (let column = Math.floor(left); column < Math.min(source.width, Math.ceil(right)); column++) {
                    const offset = (row * source.width + column) * 4;
                    const weight = rowWeight * (Math.min(column + 1, right) - Math.max(column, left));
                    const coverage = ((pixels[offset + 3] ?? 0) / 255) * weight;
                    alpha += coverage;
                    red += (linearSrgb[pixels[offset] ?? 0] ?? 0) * coverage;
                    green += (linearSrgb[pixels[offset + 1] ?? 0] ?? 0) * coverage;
                    blue += (linearSrgb[pixels[offset + 2] ?? 0] ?? 0) * coverage;
                }
            }
            const destination = (y * width + x) * 4;
            if (alpha > 0) {
                output[destination] = encodeColor(red / alpha);
                output[destination + 1] = encodeColor(green / alpha);
                output[destination + 2] = encodeColor(blue / alpha);
            }
            output[destination + 3] = Math.round((alpha / ((right - left) * (bottom - top))) * 255);
        }
    }
    return encodeCanvasRaster(width, height, 'rgba8', output);
}
