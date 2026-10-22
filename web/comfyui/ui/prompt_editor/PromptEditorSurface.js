const owners = new WeakMap();
/** Register the focused component boundary before native Cube cards mount. */
export function registerPromptEditorSurfaceOwner(document, owner) {
    const previous = owners.get(document);
    if (previous && previous !== owner)
        previous.dispose();
    owners.set(document, owner);
    return () => {
        if (owners.get(document) === owner)
            owners.delete(document);
    };
}
/** Mount through an identified renderer boundary; tests and uninitialized hosts retain native controls. */
export function mountPromptEditorSurface(surface) {
    return owners.get(surface.textarea.ownerDocument)?.mount(surface) ?? null;
}
/** Refresh only the known prompt after its native renderer adopts host state. */
export function refreshPromptEditorSurface(surface) {
    owners.get(surface.textarea.ownerDocument)?.refresh(surface);
}
