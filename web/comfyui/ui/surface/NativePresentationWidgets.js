const additions = new WeakMap();
/** Retain a native widget only for the lifetime of its presentation owner. */
export function registerNativePresentationWidget(node, widget) {
    let widgets = additions.get(node);
    if (!widgets) {
        widgets = new Set();
        additions.set(node, widgets);
    }
    widgets.add(widget);
    return () => {
        widgets.delete(widget);
        if (!widgets.size)
            additions.delete(node);
    };
}
/** Read actual graph fields even while a synchronous renderer temporarily borrows presentation additions. */
export function nativeGraphWidgets(node) {
    const presentation = additions.get(node);
    return (node.widgets ?? []).filter((widget) => !presentation?.has(widget));
}
/** Resolve native graph widgets and active presentation additions without changing the graph. */
export function nativePresentationWidgets(node) {
    return [...new Set([...(node.widgets ?? []), ...(additions.get(node) ?? [])])];
}
/** Expose additions synchronously to Comfy's extractor, then restore the exact live widget list. */
export function withNativePresentationWidgets(node, operation) {
    if (!additions.get(node)?.size)
        return operation();
    const owned = Object.hasOwn(node, 'widgets'), previous = node.widgets, items = previous ? [...previous] : null;
    node.widgets = nativePresentationWidgets(node);
    try {
        return operation();
    }
    finally {
        // Comfy's reactive widget setter may splice the original array instead of replacing it.
        if (previous && items)
            previous.splice(0, previous.length, ...items);
        if (previous)
            node.widgets = previous;
        else
            delete node.widgets;
        if (!owned)
            delete node.widgets;
    }
}
