/** Normalize the current host selection annotation at the DOM adapter boundary. */
export function readSettingsCategory(root) {
    const element = root.querySelector('[data-nav-id][class*="surface-selected"], [data-nav-id][aria-current="page"]');
    const id = element?.dataset.navId;
    return element && id ? { id, element } : null;
}
/** Send one native navigation intent and let Comfy mount the requested page. */
export function activateSettingsCategory(root, id) {
    const entry = [...root.querySelectorAll('[data-nav-id]')].find((element) => element.dataset.navId === id);
    if (!entry)
        return false;
    entry.click();
    return true;
}
