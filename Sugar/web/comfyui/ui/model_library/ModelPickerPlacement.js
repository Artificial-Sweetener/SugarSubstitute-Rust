const PREFERRED_WIDTH = 560;
const PREFERRED_HEIGHT = 630;
const MARGIN = 8;
/** Drop the margin when it would consume the whole boundary, as the Qt reference does. */
function safeAxis(origin, length) {
    const bounded = Math.max(0, length);
    return bounded > MARGIN * 2
        ? [origin + MARGIN, origin + bounded - MARGIN]
        : [origin, origin + bounded];
}
/** Prefer a full attached side, then fit the larger side without imposing overflow-causing minima. */
export function modelPickerPlacement(boundary, anchor) {
    const [safeLeft, safeRight] = safeAxis(boundary.left, boundary.width);
    const [safeTop, safeBottom] = safeAxis(boundary.top, boundary.height);
    const width = Math.min(PREFERRED_WIDTH, Math.max(1, safeRight - safeLeft));
    const left = Math.max(safeLeft, Math.min(anchor.left, safeRight - width));
    const bottom = anchor.top + anchor.height;
    const below = Math.max(0, safeBottom - bottom);
    const above = Math.max(0, anchor.top - safeTop);
    const mode = below >= PREFERRED_HEIGHT
        ? 'below'
        : above >= PREFERRED_HEIGHT || above > below
            ? 'above'
            : 'below';
    const height = Math.min(PREFERRED_HEIGHT, Math.max(1, mode === 'below' ? below : above));
    return { left, top: mode === 'below' ? bottom : anchor.top - height, width, height, mode };
}
