/** Operations in the order the operation selector lists them. */
export const CANVAS_COVERAGE_OPERATIONS = Object.freeze([
    'expand',
    'contract',
    'feather',
]);
/** Every modification editor opens on Expand by four pixels. */
export const DEFAULT_COVERAGE_MODIFICATION = Object.freeze({
    operation: 'expand',
    pixels: 4,
});
/** Smallest modification amount in whole pixels. */
export const COVERAGE_PIXELS_MIN = 1;
/** Largest modification amount in whole pixels. */
export const COVERAGE_PIXELS_MAX = 999;
/** Read an edited amount as a whole pixel count within 1..999; unreadable text keeps the last. */
export function coveragePixels(text, previous) {
    const value = Number.parseInt(text, 10);
    if (!Number.isFinite(value))
        return previous;
    return Math.min(COVERAGE_PIXELS_MAX, Math.max(COVERAGE_PIXELS_MIN, value));
}
/** Validate a coverage operation chosen in a control. */
export function coverageOperation(value) {
    return CANVAS_COVERAGE_OPERATIONS.find((operation) => operation === value) ?? null;
}
/** Brush limits: size in document pixels, hardness and opacity in percent. */
export const BRUSH_LIMITS = Object.freeze({
    size: { min: 1, max: 1000 },
    hardness: { min: 0, max: 100 },
    opacity: { min: 0, max: 100 },
});
/** Replace one brush option with a whole number inside its limits. */
export function withBrushOption(options, field, value) {
    const { min, max } = BRUSH_LIMITS[field];
    const bounded = Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : min;
    return { ...options, [field]: bounded };
}
