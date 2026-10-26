import { usesNativeMaskChannels, nativeMaskOutputChannel, } from './InputLoader.js';
import { decodeCanvasRaster, encodeCanvasRaster, canvasRasterByteLength, } from '../../core/embedded/CanvasRuntime.js';
/** Give geometric selection coverage one fixed image representation independent of loader settings. */
export function maskSourceImage(source) {
    if (source.format === 'rgba8')
        return source;
    const size = canvasRasterByteLength(source.width, source.height, 'rgba8');
    const values = decodeCanvasRaster(source), rgba = new Uint8Array(size);
    for (let index = 0; index < values.length; index++) {
        const value = values[index] ?? 0, offset = index * 4;
        rgba[offset] = rgba[offset + 1] = rgba[offset + 2] = value;
        rgba[offset + 3] = 255 - value;
    }
    return encodeCanvasRaster(source.width, source.height, 'rgba8', rgba);
}
/** Match Comfy's RGB extraction and inverse-alpha mask semantics without changing the input image. */
export function interpretMaskSource(source, channel) {
    if (!['red', 'green', 'blue', 'alpha'].includes(channel))
        throw new Error('Unsupported mask channel.');
    const image = maskSourceImage(source), values = decodeCanvasRaster(image), coverage = new Uint8Array(image.width * image.height);
    const component = channel === 'alpha' ? 3 : channel === 'green' ? 1 : channel === 'blue' ? 2 : 0;
    for (let index = 0; index < coverage.length; index++) {
        const value = values[index * 4 + component] ?? 0;
        coverage[index] = channel === 'alpha' ? 255 - value : value;
    }
    return encodeCanvasRaster(image.width, image.height, 'coverage8', coverage);
}
/** Interpret known native coverage once and leave custom loader semantics to their native codec. */
export function nativeMaskSource(source, loader, settings, imageMaskOutput) {
    const channel = imageMaskOutput ? nativeMaskOutputChannel(loader) : settings.inputs.channel;
    if (!imageMaskOutput && !usesNativeMaskChannels(loader))
        return null;
    if (imageMaskOutput && channel === null)
        return null;
    if (typeof channel !== 'string')
        throw new Error('The mask channel must be an authored setting.');
    return interpretMaskSource(source, channel);
}
