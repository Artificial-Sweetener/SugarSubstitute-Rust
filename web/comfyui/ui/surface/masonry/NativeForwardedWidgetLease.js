/** Preserve Cube boundary ownership when Comfy reuses an internal DOM element for a promoted widget. */
export class NativeForwardedWidgetLease {
    #release = [];
    /** Observe native edits after their own handler, without replacing or cloning controls. */
    constructor(target, node, sources) {
        for (const source of sources) {
            const internal = node.widgets?.find((widget) => widget.name === source.targetName);
            const owner = source.node.widgets?.find((widget) => widget.name === source.name);
            if (!internal || !owner)
                continue;
            let lastValue = internal.value;
            let active = true;
            const forward = () => {
                if (!active || lastValue === internal.value)
                    return;
                lastValue = internal.value;
                const previous = owner.value;
                if (previous === internal.value)
                    return;
                owner.value = internal.value;
                owner.callback?.(owner.value);
                const changed = source.node.onWidgetChanged;
                if (typeof changed === 'function')
                    changed.call(source.node, source.name, owner.value, previous, owner);
            };
            const afterNativeEdit = () => queueMicrotask(() => queueMicrotask(forward));
            target.addEventListener('input', afterNativeEdit, true);
            target.addEventListener('change', afterNativeEdit, true);
            this.#release.push(() => {
                forward();
                active = false;
                target.removeEventListener('input', afterNativeEdit, true);
                target.removeEventListener('change', afterNativeEdit, true);
            });
        }
    }
    /** Remove forwarding before the native widget returns to graph presentation. */
    dispose() {
        for (const release of this.#release.splice(0))
            release();
    }
}
