/** Accept a scroll from storage or a caller: a bounded offset and a Cube id or none. */
export function isWorkflowScroll(value) {
    if (typeof value !== 'object' || value === null)
        return false;
    const { cube, offset } = value;
    return ((cube === null || (typeof cube === 'string' && cube.length > 0 && cube.length <= 256)) &&
        typeof offset === 'number' &&
        Number.isFinite(offset) &&
        offset >= 0 &&
        offset <= 10_000_000);
}
