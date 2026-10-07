/** Traverse open shadow boundaries as part of the same visual scroll hierarchy. */
export function scrollParent(element) {
    const root = element.getRootNode();
    return (element.parentElement ??
        (root instanceof ShadowRoot && root.host instanceof HTMLElement ? root.host : null));
}
/** Clip a scroll viewport without changing its box or inserting layout wrappers. */
export function scrollbarTrack(element) {
    const windowRef = element.ownerDocument.defaultView;
    if (!windowRef || !element.isConnected)
        return null;
    const root = element === element.ownerDocument.scrollingElement;
    const rect = element.getBoundingClientRect();
    const scaleX = element.offsetWidth ? rect.width / element.offsetWidth : 1;
    const scaleY = element.offsetHeight ? rect.height / element.offsetHeight : 1;
    let left = root ? 0 : rect.left + element.clientLeft * scaleX;
    let top = root ? 0 : rect.top + element.clientTop * scaleY;
    let right = root
        ? windowRef.innerWidth
        : Math.min(rect.right, left + element.clientWidth * scaleX);
    let bottom = root
        ? windowRef.innerHeight
        : Math.min(rect.bottom, top + element.clientHeight * scaleY);
    const edge = element.getAttribute('data-sc-scrollbar-edge') === 'parent' ? scrollParent(element) : null;
    if (edge)
        right = edge.getBoundingClientRect().right - 1;
    for (let parent = scrollParent(element); parent && !root; parent = scrollParent(parent)) {
        const style = windowRef.getComputedStyle(parent);
        const bounds = parent.getBoundingClientRect();
        if (/(auto|scroll|hidden|clip)/.test(style.overflowX)) {
            left = Math.max(left, bounds.left);
            right = Math.min(right, bounds.right);
        }
        if (/(auto|scroll|hidden|clip)/.test(style.overflowY)) {
            top = Math.max(top, bounds.top);
            bottom = Math.min(bottom, bounds.bottom);
        }
    }
    left = Math.max(0, left);
    top = Math.max(0, top);
    right = Math.min(windowRef.innerWidth, right);
    bottom = Math.min(windowRef.innerHeight, bottom);
    if (right - left <= 12 || bottom - top <= 12)
        return null;
    return {
        left,
        top,
        width: right - left,
        height: bottom - top,
        rtl: windowRef.getComputedStyle(element).direction === 'rtl',
    };
}
