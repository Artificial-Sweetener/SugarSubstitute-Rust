/** Require both finite coordinates while accepting native typed vectors. */
export function readNativeCubePosition(value) {
    if (!Array.isArray(value) && !(value instanceof Float32Array) && !(value instanceof Float64Array))
        throw new TypeError('Cube reorder position must be a native coordinate vector.');
    const [x, y] = value;
    if (typeof x !== 'number' || typeof y !== 'number' || !Number.isFinite(x) || !Number.isFinite(y))
        throw new TypeError('Cube reorder position must contain finite coordinates.');
    return [x, y];
}
/** Use the native setter for geometry notification and retain the existing coordinate vector. */
export function writeNativeCubePosition(node, position) {
    node.setPos?.(position[0], position[1]);
    restoreNativeCubePosition(node, position);
}
/** Restore coordinates without invoking a setter that has already failed. */
export function restoreNativeCubePosition(node, position) {
    if (!node.pos)
        throw new TypeError('Cube reorder target has no native position.');
    node.pos[0] = position[0];
    node.pos[1] = position[1];
}
