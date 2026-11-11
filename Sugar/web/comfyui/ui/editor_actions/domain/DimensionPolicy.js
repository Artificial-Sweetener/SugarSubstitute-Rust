/** Retain Substitute's landscape ratio order. */
export const LANDSCAPE_RATIOS = [
    [1, 1],
    [5, 4],
    [4, 3],
    [3, 2],
    [16, 9],
    [2, 1],
    [21, 9],
];
/** Retain Substitute's portrait ratio order. */
export const PORTRAIT_RATIOS = [
    [1, 1],
    [4, 5],
    [3, 4],
    [2, 3],
    [9, 16],
    [1, 2],
    [9, 21],
];
/** Match exact width/height suffixes without joining unrelated source and target fields. */
export function inferDimensionPairs(names) {
    const candidates = new Map();
    for (const name of names) {
        const match = /^(?:(.+)_)?(width|height)$/.exec(name);
        if (!match)
            continue;
        const stem = match[1] ?? '';
        const side = match[2] === 'width' ? 'width' : 'height';
        const candidate = candidates.get(stem) ?? {};
        candidate[side] ??= name;
        candidates.set(stem, candidate);
    }
    return [...candidates].flatMap(([stem, pair]) => pair.width && pair.height ? [{ stem, width: pair.width, height: pair.height }] : []);
}
/** Match Python's ties-to-even rounding used by Substitute's dimension commands. */
export function roundDimension(value) {
    const lower = Math.floor(value);
    return value - lower === 0.5 ? lower + Math.abs(lower % 2) : Math.round(value);
}
/** Calculate only the other edge; widget constraints remain with the native field owner. */
export function ratioDimension(anchor, side, width, height) {
    if (![anchor, width, height].every(Number.isFinite) || width <= 0 || height <= 0)
        return null;
    const result = roundDimension(side === 'width' ? (anchor * height) / width : (anchor * width) / height);
    return Number.isSafeInteger(result) ? result : null;
}
/** Normalize positive integer dimensions into orientation-independent persisted edges. */
export function canonicalDimensions(width, height) {
    const edges = [roundDimension(width), roundDimension(height)];
    if (!edges.every((edge) => Number.isSafeInteger(edge) && edge > 0))
        return null;
    return { short_edge: Math.min(...edges), long_edge: Math.max(...edges) };
}
