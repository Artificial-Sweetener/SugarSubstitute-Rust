/** Capture the user's settled position relative to the nearest available workflow edge. */
export function captureOverrideToolbarAnchor(position, bounds, size) {
    const edges = [
        { edge: 'left', distance: position.x - bounds.left },
        { edge: 'right', distance: bounds.left + bounds.width - position.x - size.width },
        { edge: 'top', distance: position.y - bounds.top },
        { edge: 'bottom', distance: bounds.top + bounds.height - position.y - size.height },
    ];
    const nearest = edges.reduce((best, candidate) => candidate.distance < best.distance ? candidate : best);
    return {
        edge: nearest.edge,
        distance: Math.max(0, nearest.distance),
        horizontalRatio: Math.max(0, Math.min(1, (position.x - bounds.left) / Math.max(1, bounds.width))),
        verticalRatio: Math.max(0, Math.min(1, (position.y - bounds.top) / Math.max(1, bounds.height))),
    };
}
/** Resolve remembered placement against current bounds while retaining its edge distance. */
export function resolveOverrideToolbarAnchor(anchor, bounds, size) {
    let x = bounds.left + anchor.horizontalRatio * bounds.width;
    let y = bounds.top + anchor.verticalRatio * bounds.height;
    if (anchor.edge === 'left')
        x = bounds.left + anchor.distance;
    else if (anchor.edge === 'right')
        x = bounds.left + bounds.width - size.width - anchor.distance;
    else if (anchor.edge === 'top')
        y = bounds.top + anchor.distance;
    else
        y = bounds.top + bounds.height - size.height - anchor.distance;
    return { x, y };
}
