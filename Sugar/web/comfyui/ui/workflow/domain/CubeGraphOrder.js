/** Keep every Cube eligible regardless of ports, execution dependencies or segment membership. */
export function cubeGraphOrder(analysis) {
    return cubePlacementOrder(analysis.placements);
}
/** Apply canonical position ordering to native placements before or after backend analysis. */
export function cubePlacementOrder(placements) {
    return [...placements]
        .sort((left, right) => left.position[0] - right.position[0] ||
        left.position[1] - right.position[1] ||
        (left.instanceId < right.instanceId ? -1 : left.instanceId > right.instanceId ? 1 : 0))
        .map((placement) => placement.instanceId);
}
