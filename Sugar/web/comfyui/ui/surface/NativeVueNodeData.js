const widgetDescriptors = new WeakMap();
/** Restore only a descriptor installed by this boundary; external replacements become the next authoritative base. */
function sameDescriptor(left, right) {
    return (left === right ||
        (!!left &&
            !!right &&
            left.get === right.get &&
            left.set === right.set &&
            left.value === right.value &&
            left.writable === right.writable &&
            left.configurable === right.configurable &&
            left.enumerable === right.enumerable));
}
/** Invoke native extraction through the sole widget descriptor adaptation boundary. */
export function extractNativeVueNodeData(node, extract) {
    const previous = widgetDescriptors.get(node);
    let base = Object.getOwnPropertyDescriptor(node, 'widgets');
    if (previous && sameDescriptor(base, previous.installed)) {
        base = previous.base;
        if (base) {
            if ('value' in base) {
                const widgets = Reflect.get(node, 'widgets');
                base = { ...base, value: widgets };
            }
            Object.defineProperty(node, 'widgets', base);
        }
        else if (!Reflect.deleteProperty(node, 'widgets'))
            throw new TypeError('Cannot restore native widget ownership before extraction.');
    }
    try {
        return extract(node);
    }
    finally {
        widgetDescriptors.set(node, {
            base,
            installed: Object.getOwnPropertyDescriptor(node, 'widgets'),
        });
    }
}
