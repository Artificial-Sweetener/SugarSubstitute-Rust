/** Integrate a Lanczos-3 footprint scaled to the source-to-display ratio. */
function weights(source, target) {
    const scale = Math.max(1, source / target);
    return Array.from({ length: target }, (_, pixel) => {
        const center = ((pixel + 0.5) * source) / target - 0.5;
        const taps = [];
        let total = 0;
        for (let index = Math.max(0, Math.ceil(center - 3 * scale)); index <= Math.min(source - 1, Math.floor(center + 3 * scale)); index++) {
            const distance = (center - index) / scale;
            const radians = Math.PI * distance;
            const weight = Math.abs(distance) < 1e-9
                ? 1
                : Math.abs(distance) >= 3
                    ? 0
                    : ((Math.sin(radians) / radians) * Math.sin(radians / 3)) / (radians / 3);
            taps.push({ index, weight });
            total += weight;
        }
        return taps.map((tap) => ({ index: tap.index, weight: tap.weight / total }));
    });
}
/** Preserve fine detail and transparent-edge color while reducing the complete glass composition once. */
export function resampleOrbRaster(pixels, source, target) {
    if (!Number.isInteger(source) ||
        !Number.isInteger(target) ||
        source < 1 ||
        target < 1 ||
        pixels.length !== source * source * 4)
        throw new RangeError('Orb raster dimensions do not match the RGBA source.');
    if (source === target)
        return pixels.slice();
    const taps = weights(source, target);
    const rows = new Float64Array(source * target * 4);
    for (let y = 0; y < source; y++)
        for (let x = 0; x < target; x++) {
            const destination = (y * target + x) * 4;
            for (const tap of taps[x] ?? []) {
                const offset = (y * source + tap.index) * 4;
                const alpha = (pixels[offset + 3] ?? 0) / 255;
                for (let channel = 0; channel < 4; channel++)
                    rows[destination + channel] =
                        (rows[destination + channel] ?? 0) +
                            (pixels[offset + channel] ?? 0) * (channel === 3 ? 1 : alpha) * tap.weight;
            }
        }
    const result = new Uint8ClampedArray(target * target * 4);
    for (let y = 0; y < target; y++)
        for (let x = 0; x < target; x++) {
            const rgba = [0, 0, 0, 0];
            for (const tap of taps[y] ?? [])
                for (let channel = 0; channel < 4; channel++)
                    rgba[channel] =
                        (rgba[channel] ?? 0) + (rows[(tap.index * target + x) * 4 + channel] ?? 0) * tap.weight;
            const offset = (y * target + x) * 4;
            const alpha = Math.max(0, Math.min(255, rgba[3] ?? 0));
            for (let channel = 0; channel < 3; channel++)
                result[offset + channel] = alpha > 0 ? ((rgba[channel] ?? 0) * 255) / alpha : 0;
            result[offset + 3] = alpha;
        }
    return result;
}
/** Resolve the 46 CSS-pixel orb's backing store at the accepted monitor/browser density. */
export function orbPhysicalEdge(density) {
    return Math.max(1, Math.round(46 * (Number.isFinite(density) && density > 0 ? density : 1)));
}
