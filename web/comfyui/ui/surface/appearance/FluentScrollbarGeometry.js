/** Keep a bounded minimum handle and clamp overscroll to the authoritative range. */
export function scrollbarGeometry(track, viewport, content, position) {
    if (![track, viewport, content, position].every(Number.isFinite))
        return null;
    if (track <= 0 || viewport <= 0 || content <= viewport)
        return null;
    const maximum = content - viewport;
    const length = Math.min(track, Math.max(24, (track * viewport) / content));
    const travel = track - length;
    return {
        length,
        travel,
        maximum,
        offset: (Math.min(maximum, Math.max(0, position)) / maximum) * travel,
    };
}
/** Map pointer movement to scrolling without dividing by an immovable handle. */
export function scrollbarDragPosition(geometry, initial, distance) {
    if (geometry.travel <= 0)
        return initial;
    return Math.min(geometry.maximum, Math.max(0, initial + (distance / geometry.travel) * geometry.maximum));
}
