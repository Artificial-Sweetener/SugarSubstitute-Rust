const hiddenWidgets = new WeakMap();
/** Read transient presentation without excluding a widget from semantic discovery. */
export function isNativeWidgetPresented(widget) {
    return !hiddenWidgets.has(widget);
}
/** Count a locally replaced native field as content while its replacement controls mount or retire. */
export function isNativeWidgetLocallyRepresented(widget) {
    const owner = hiddenWidgets.get(widget);
    return !owner || owner.disposition === 'replacement';
}
/** Own one scope of widget visibility leases without altering serialization or native controls. */
export class NativeWidgetPresentation {
    disposition;
    #hidden = new Set();
    /** Distinguish fields moved elsewhere from fields whose local replacement retains their card. */
    constructor(disposition = 'external') {
        this.disposition = disposition;
    }
    /** Apply the next resolved presentation and return whether native layout needs refreshing. */
    reconcile(widgets) {
        let changed = false;
        for (const widget of this.#hidden)
            if (!widgets.has(widget)) {
                if (hiddenWidgets.get(widget) === this)
                    hiddenWidgets.delete(widget);
                this.#hidden.delete(widget);
                changed = true;
            }
        for (const widget of widgets)
            if (!this.#hidden.has(widget)) {
                const owner = hiddenWidgets.get(widget);
                if (owner && owner !== this)
                    throw new Error('Native widget visibility has competing owners.');
                hiddenWidgets.set(widget, this);
                this.#hidden.add(widget);
                changed = true;
            }
        return changed;
    }
    /** Restore presentation immediately when a workflow or owning host is released. */
    dispose() {
        this.reconcile(new Set());
    }
}
