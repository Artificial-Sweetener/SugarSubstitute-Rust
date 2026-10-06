/** Remember widget DOM ownership without copying controls or changing their callbacks. */
export class NativeWidgetElementLease {
    #entries = [];
    /** Capture graph-rendered singleton elements before Comfy adopts them into masonry. */
    constructor(nodes) {
        for (const node of nodes) {
            for (const widget of node.widgets ?? []) {
                const element = widget.element;
                if (element instanceof HTMLElement && element.parentNode) {
                    this.#entries.push({ element, parent: element.parentNode, next: element.nextSibling });
                }
            }
        }
    }
    /** Return surviving widgets to their original graph mounts after native unmount completes. */
    restore() {
        for (const { element, parent, next } of this.#entries) {
            if (!parent.isConnected)
                continue;
            parent.insertBefore(element, next?.parentNode === parent ? next : null);
        }
        this.#entries.length = 0;
    }
}
