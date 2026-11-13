const additions = new WeakMap();
/** Retain a presentation widget with optional placement before an existing graph field. */
export function registerNativePresentationWidget(node, widget, beforeField = null) {
    let widgets = additions.get(node);
    if (!widgets) {
        widgets = new Map();
        additions.set(node, widgets);
    }
    widgets.set(widget, beforeField);
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
/** Resolve presentation additions before their optional native field anchor without changing graph order. */
export function nativePresentationWidgets(node) {
    const widgets = [...new Set(nativeGraphWidgets(node))];
    for (const [widget, beforeField] of additions.get(node) ?? []) {
        const index = beforeField ? widgets.findIndex((item) => item.name === beforeField) : -1;
        if (index < 0)
            widgets.push(widget);
        else
            widgets.splice(index, 0, widget);
    }
    return widgets;
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
