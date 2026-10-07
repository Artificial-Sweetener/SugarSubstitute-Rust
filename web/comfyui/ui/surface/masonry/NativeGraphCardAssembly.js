/** Retain the real graph layout while it blends into its native masonry presentation during travel. */
export class NativeGraphCardAssembly {
    source;
    #transform;
    #origin;
    #identities;
    #attached = false;
    /** Capture stable graph geometry before any effect or canonical-observer suspension. */
    constructor(source) {
        this.source = source;
        const element = source.element;
        this.#transform =
            element.ownerDocument.defaultView?.getComputedStyle(element).transform ?? 'none';
        this.#origin = {
            value: element.style.getPropertyValue('transform-origin'),
            priority: element.style.getPropertyPriority('transform-origin'),
        };
        this.#identities = [
            element,
            ...element.querySelectorAll('[data-widgets-grid-node-id]'),
        ].flatMap((target) => ['data-node-id', 'data-widgets-grid-node-id'].flatMap((name) => {
            const value = target.getAttribute(name);
            return value === null ? [] : [{ element: target, name, value }];
        }));
    }
    /** Interpolate native layout width and camera scale while preserving the host's node translation. */
    frames(destination, entering, interrupted) {
        const { element, rectangle, scale, layoutWidth } = this.source;
        const computed = interrupted
            ? element.ownerDocument.defaultView?.getComputedStyle(element)
            : null;
        const graphTransform = this.#transform === 'none' ? 'translate(0px, 0px) scale(1)' : this.#transform;
        const base = this.#transform === 'none' ? '' : `${this.#transform} `;
        const masonryTransform = `${base}translate(${String((destination.x - rectangle.x) / scale)}px, ${String((destination.y - rectangle.y) / scale)}px) scale(${String(1 / scale)})`;
        return [
            {
                transform: computed?.transform || (entering ? graphTransform : masonryTransform),
                width: computed?.width || `${String(entering ? layoutWidth : destination.width)}px`,
            },
            {
                transform: entering ? masonryTransform : graphTransform,
                width: `${String(entering ? destination.width : layoutWidth)}px`,
            },
        ];
    }
    /** Suspend only the host's DOM geometry identities for the duration of the width interpolation. */
    attach() {
        if (this.#attached)
            return;
        this.#attached = true;
        const element = this.source.element;
        element.dataset.scMasonryGraphNode = element.dataset.nodeId ?? '';
        element.style.setProperty('transform-origin', '0 0');
        // Comfy's shared ResizeObserver resolves canonical node bounds through these DOM attributes.
        for (const identity of this.#identities)
            identity.element.removeAttribute(identity.name);
    }
    /** Restore host identities after all effects have returned the original native dimensions. */
    dispose() {
        if (!this.#attached)
            return;
        this.#attached = false;
        const element = this.source.element;
        delete element.dataset.scMasonryGraphNode;
        if (this.#origin.value)
            element.style.setProperty('transform-origin', this.#origin.value, this.#origin.priority);
        else
            element.style.removeProperty('transform-origin');
        for (const identity of this.#identities)
            if (!identity.element.hasAttribute(identity.name))
                identity.element.setAttribute(identity.name, identity.value);
    }
}
