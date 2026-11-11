const boundaries = new WeakMap();
/** Bind the renderer's actual node until its card mount is released. */
export function registerNativeNodeCard(root, node, cubeId = null) {
    const boundary = { node, cubeId };
    boundaries.set(root, boundary);
    return () => {
        if (boundaries.get(root) === boundary)
            boundaries.delete(root);
    };
}
/** Resolve the nearest positively registered card through graph and masonry relocation. */
export function readNativeNodeCard(element) {
    for (let root = element; root; root = root.parentElement) {
        const boundary = boundaries.get(root);
        if (boundary)
            return boundary;
    }
    return null;
}
