const mounts = new WeakMap();
/** Register the actual Vue mount and optional seed value owner for shared appearance. */
export function registerNativeWidgetMount(root, boundary) {
    mounts.set(root, boundary);
}
/** Resolve only positively registered native widget mounts. */
export function readNativeWidgetMount(root) {
    return mounts.get(root) ?? null;
}
/** Release renderer ownership before removing its DOM. */
export function releaseNativeWidgetMount(root) {
    mounts.delete(root);
}
